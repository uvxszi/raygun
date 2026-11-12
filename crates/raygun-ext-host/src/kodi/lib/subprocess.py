"""`subprocess` for Kodi add-ons in Raygun: no programs can be started."""

PIPE = -1
STDOUT = -2
DEVNULL = -3


class SubprocessError(Exception):
    pass


class CalledProcessError(SubprocessError):
    def __init__(self, returncode, cmd, output=None, stderr=None):
        self.returncode = returncode
        self.cmd = cmd
        self.output = output
        self.stdout = output
        self.stderr = stderr


class TimeoutExpired(SubprocessError):
    def __init__(self, cmd, timeout, output=None, stderr=None):
        self.cmd = cmd
        self.timeout = timeout
        self.output = output
        self.stdout = output
        self.stderr = stderr


class CompletedProcess:
    def __init__(self, args, returncode, stdout=None, stderr=None):
        self.args = args
        self.returncode = returncode
        self.stdout = stdout
        self.stderr = stderr


def _refused():
    return PermissionError(1, 'Kodi add-ons in Raygun can\'t start programs')


class Popen:
    def __init__(self, *args, **kwargs):
        raise _refused()


def run(*args, **kwargs):
    raise _refused()


def call(*args, **kwargs):
    raise _refused()


def check_call(*args, **kwargs):
    raise _refused()


def check_output(*args, **kwargs):
    raise _refused()


def getoutput(cmd):
    raise _refused()


def getstatusoutput(cmd):
    raise _refused()


def list2cmdline(seq):
    return ' '.join(seq)
