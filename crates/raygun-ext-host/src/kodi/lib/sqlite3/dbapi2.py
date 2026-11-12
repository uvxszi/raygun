"""DB-API 2.0 over `_raygun_sqlite` (a statement at a time, rows read at once), with the parts of
CPython's `sqlite3` add-ons use: rows by name, adapters and converters, transactions begun before
changes as the standard library's legacy mode does."""

import datetime
import time

import _raygun_sqlite

apilevel = '2.0'
threadsafety = 1
paramstyle = 'qmark'
version = '2.6.0'
version_info = (2, 6, 0)
sqlite_version = _raygun_sqlite.version()
sqlite_version_info = tuple(int(part) for part in sqlite_version.split('.'))
PARSE_DECLTYPES = 1
PARSE_COLNAMES = 2
SQLITE_OK = 0
SQLITE_DENY = 1
SQLITE_IGNORE = 2
LEGACY_TRANSACTION_CONTROL = -1


class Warning(Exception):
    pass


class Error(Exception):
    sqlite_errorcode = 1
    sqlite_errorname = 'SQLITE_ERROR'


class InterfaceError(Error):
    pass


class DatabaseError(Error):
    pass


class DataError(DatabaseError):
    pass


class OperationalError(DatabaseError):
    pass


class IntegrityError(DatabaseError):
    pass


class InternalError(DatabaseError):
    pass


class ProgrammingError(DatabaseError):
    pass


class NotSupportedError(DatabaseError):
    pass


Binary = memoryview
Date = datetime.date
Time = datetime.time
Timestamp = datetime.datetime


def DateFromTicks(ticks):
    return Date(*time.localtime(ticks)[:3])


def TimeFromTicks(ticks):
    return Time(*time.localtime(ticks)[3:6])


def TimestampFromTicks(ticks):
    return Timestamp(*time.localtime(ticks)[:6])


_adapters = {}
_converters = {}


def register_adapter(kind, adapter):
    _adapters[kind] = adapter


def register_converter(name, converter):
    _converters[name.upper()] = converter


def adapt(value, proto=None, alt=None):
    adapter = _adapters.get(type(value))
    return adapter(value) if adapter else value


def complete_statement(statement):
    return statement.strip().endswith(';')


def enable_callback_tracebacks(flag):
    pass


def _convert_timestamp(value):
    text = value.decode() if isinstance(value, bytes) else value
    date_part, _, time_part = text.partition(' ')
    year, month, day = map(int, date_part.split('-'))
    if not time_part:
        return datetime.datetime(year, month, day)
    clock, _, fraction = time_part.partition('.')
    hours, minutes, seconds = map(int, clock.split(':'))
    micro = int('{:0<6.6}'.format(fraction)) if fraction else 0
    return datetime.datetime(year, month, day, hours, minutes, seconds, micro)


register_adapter(datetime.date, lambda value: value.isoformat())
register_adapter(datetime.datetime, lambda value: value.isoformat(' '))
register_converter('date', lambda value: datetime.date(*map(int, value.split(b'-'))))
register_converter('timestamp', _convert_timestamp)


def _error(code, message):
    code = code or 1
    primary = code & 0xff
    if primary == 19:
        kind = IntegrityError
    elif primary in (21,):
        kind = ProgrammingError
    elif primary in (11, 26):
        kind = DatabaseError
    elif primary == 20:
        kind = DataError
    else:
        kind = OperationalError
    raised = kind(message)
    raised.sqlite_errorcode = code
    return raised


def _bind(value):
    adapter = _adapters.get(type(value))
    if adapter is not None:
        value = adapter(value)
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, (memoryview, bytearray)):
        return bytes(value)
    if value is None or isinstance(value, (int, float, str, bytes)):
        return value
    if hasattr(value, '__conform__'):
        return value.__conform__(PrepareProtocol)
    raise ProgrammingError('Error binding parameter: type %r is not supported'
                           % type(value).__name__)


class PrepareProtocol:
    pass


class Row:
    def __init__(self, cursor, values):
        self._names = [entry[0] for entry in (cursor.description or ())]
        self._values = tuple(values)

    def keys(self):
        return list(self._names)

    def __getitem__(self, key):
        if isinstance(key, (int, slice)):
            return self._values[key]
        lowered = key.lower()
        for index, name in enumerate(self._names):
            if name.lower() == lowered:
                return self._values[index]
        raise IndexError('No item with that key')

    def __iter__(self):
        return iter(self._values)

    def __len__(self):
        return len(self._values)

    def __eq__(self, other):
        return (isinstance(other, Row) and self._names == other._names
                and self._values == other._values)

    def __hash__(self):
        return hash((tuple(self._names), self._values))

    def __repr__(self):
        return '<sqlite3.Row %r>' % (self._values,)


class Cursor:
    def __init__(self, connection):
        self.connection = connection
        self.description = None
        self.rowcount = -1
        self.lastrowid = None
        self.arraysize = 1
        self.row_factory = None
        self._rows = []
        self._position = 0
        self._closed = False

    def __iter__(self):
        return self

    def __next__(self):
        row = self.fetchone()
        if row is None:
            raise StopIteration
        return row

    def _check(self):
        if self._closed:
            raise ProgrammingError('Cannot operate on a closed cursor.')
        self.connection._check()

    def execute(self, sql, parameters=()):
        self._check()
        if isinstance(parameters, dict):
            bound = {key: _bind(value) for key, value in parameters.items()}
        else:
            bound = [_bind(value) for value in (parameters or ())]
        self.connection._begin_if_needed(sql)
        answer = _raygun_sqlite.execute(self.connection._handle, sql, bound)
        if answer[0]:
            raise _error(answer[0], answer[1])
        _, columns, rows, changes, lastrowid = answer
        self.description = tuple((name, None, None, None, None, None, None)
                                 for name, _ in columns) if columns else None
        converters = self.connection._converters_for(columns)
        if converters:
            rows = [tuple(convert(value) if (convert and value is not None) else value
                          for convert, value in zip(converters, row)) for row in rows]
        self._rows = rows
        self._position = 0
        verb = sql.lstrip().split(None, 1)[0].upper() if sql.strip() else ''
        self.rowcount = changes if verb in ('INSERT', 'UPDATE', 'DELETE', 'REPLACE') else -1
        if verb in ('INSERT', 'REPLACE'):
            self.lastrowid = lastrowid
        return self

    def executemany(self, sql, seq_of_parameters):
        total = 0
        for parameters in seq_of_parameters:
            self.execute(sql, parameters)
            if self.rowcount > 0:
                total += self.rowcount
        self.rowcount = total
        return self

    def executescript(self, script):
        self._check()
        self.connection.commit()
        answer = _raygun_sqlite.script(self.connection._handle, script)
        if answer[0]:
            raise _error(answer[0], answer[1])
        self._rows = []
        return self

    def _make(self, values):
        factory = self.row_factory or self.connection.row_factory
        if factory is None:
            return tuple(values)
        return factory(self, tuple(values))

    def fetchone(self):
        if self._position >= len(self._rows):
            return None
        row = self._rows[self._position]
        self._position += 1
        return self._make(row)

    def fetchmany(self, size=None):
        size = self.arraysize if size is None else size
        found = []
        while len(found) < size:
            row = self.fetchone()
            if row is None:
                break
            found.append(row)
        return found

    def fetchall(self):
        found = []
        while True:
            row = self.fetchone()
            if row is None:
                return found
            found.append(row)

    def setinputsizes(self, sizes):
        pass

    def setoutputsize(self, size, column=None):
        pass

    def close(self):
        self._closed = True


class Connection:
    Warning = Warning
    Error = Error
    InterfaceError = InterfaceError
    DatabaseError = DatabaseError
    DataError = DataError
    OperationalError = OperationalError
    IntegrityError = IntegrityError
    InternalError = InternalError
    ProgrammingError = ProgrammingError
    NotSupportedError = NotSupportedError

    def __init__(self, database, timeout=5.0, detect_types=0, isolation_level='',
                 check_same_thread=True, factory=None, cached_statements=128, uri=False,
                 autocommit=LEGACY_TRANSACTION_CONTROL):
        path = database
        if not isinstance(path, str):
            import os
            path = os.fsdecode(path)
        if uri and path.startswith('file:'):
            path = path[5:].split('?', 1)[0] or ':memory:'
        if path != ':memory:' and path != '':
            import posix
            path = posix._text(path)
        answer = _raygun_sqlite.open(path or ':memory:', float(timeout))
        if answer[0]:
            raise _error(answer[0], answer[1])
        self._handle = answer[1]
        self.isolation_level = isolation_level
        self.detect_types = detect_types
        self.row_factory = None
        self.text_factory = str
        self._closed = False

    def _check(self):
        if self._closed:
            raise ProgrammingError('Cannot operate on a closed database.')

    def _converters_for(self, columns):
        if not (self.detect_types & PARSE_DECLTYPES) or not _converters or not columns:
            return None
        found = []
        for _, declared in columns:
            name = (declared or '').split('(')[0].strip().split(' ')[0].upper()
            found.append(_converters.get(name))
        if not any(found):
            return None
        return [(lambda value, convert=convert: convert(
                    value if isinstance(value, bytes) else str(value).encode()))
                if convert else None for convert in found]

    def _begin_if_needed(self, sql):
        if self.isolation_level is None or self.in_transaction:
            return
        verb = sql.lstrip().split(None, 1)[0].upper() if sql.strip() else ''
        if verb in ('INSERT', 'UPDATE', 'DELETE', 'REPLACE'):
            answer = _raygun_sqlite.execute(self._handle, 'BEGIN ' + (self.isolation_level or ''), [])
            if answer[0]:
                raise _error(answer[0], answer[1])

    @property
    def in_transaction(self):
        if self._closed:
            return False
        return not _raygun_sqlite.autocommit(self._handle)

    @property
    def total_changes(self):
        return _raygun_sqlite.total_changes(self._handle)

    def cursor(self, factory=Cursor):
        self._check()
        return factory(self)

    def execute(self, sql, parameters=()):
        return self.cursor().execute(sql, parameters)

    def executemany(self, sql, seq_of_parameters):
        return self.cursor().executemany(sql, seq_of_parameters)

    def executescript(self, script):
        return self.cursor().executescript(script)

    def commit(self):
        self._check()
        if self.in_transaction:
            answer = _raygun_sqlite.execute(self._handle, 'COMMIT', [])
            if answer[0]:
                raise _error(answer[0], answer[1])

    def rollback(self):
        self._check()
        if self.in_transaction:
            answer = _raygun_sqlite.execute(self._handle, 'ROLLBACK', [])
            if answer[0]:
                raise _error(answer[0], answer[1])

    def close(self):
        if not self._closed:
            try:
                self.commit()
            except Error:
                pass
            _raygun_sqlite.close(self._handle)
            self._closed = True

    def __enter__(self):
        return self

    def __exit__(self, kind, value, traceback):
        if kind is None:
            self.commit()
        else:
            self.rollback()
        return False

    def __del__(self):
        try:
            if not self._closed:
                _raygun_sqlite.close(self._handle)
        except Exception:
            pass

    def create_function(self, name, num_params, func, *, deterministic=False):
        pass

    def create_aggregate(self, name, num_params, aggregate_class):
        pass

    def create_collation(self, name, callable):
        pass

    def set_authorizer(self, authorizer_callback):
        pass

    def set_progress_handler(self, progress_handler, n):
        pass

    def set_trace_callback(self, trace_callback):
        pass

    def interrupt(self):
        pass

    def iterdump(self):
        return iter(())

    def backup(self, target, **kwargs):
        raise NotSupportedError('backups are not available')


def connect(database, timeout=5.0, detect_types=0, isolation_level='', check_same_thread=True,
            factory=Connection, cached_statements=128, uri=False, **kwargs):
    return factory(database, timeout, detect_types, isolation_level, check_same_thread,
                   None, cached_statements, uri)
