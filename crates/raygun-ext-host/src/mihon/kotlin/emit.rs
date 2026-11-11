//! JavaScript for whole Kotlin functions, for sources written without a theme
//! (`crate::mihon::transpile`). Everything the generated code calls is asynchronous and awaited,
//! as a source's `suspend` functions are; member calls on values go through the runtime's
//! Kotlin, Jsoup and OkHttp tables (`KT.m`, `mihon/kotlin.js`), which know what a value is when
//! the code runs. Calls the runtime doesn't have are refused here, with their name, so a source
//! whose code needs them isn't converted.

use std::collections::{HashMap, HashSet};

use super::ast::*;

/// The names a source's class knows besides its locals.
#[derive(Debug, Default)]
pub struct Context {
    /// Its functions (with the base class's), each overload's parameter names.
    pub functions: HashMap<String, Vec<Vec<String>>>,
    /// Its properties (with the base class's).
    pub properties: HashSet<String>,
    /// Extension functions and properties its files declare.
    pub extensions: HashSet<String>,
    pub extension_properties: HashSet<String>,
    /// Constants its files and companion objects declare, read as `C.name`.
    pub constants: HashSet<String>,
    /// Classes and objects its files declare (filters, DTOs), made as `D.Name(...)`.
    pub classes: HashSet<String>,
    /// The methods and properties of those classes, called on their instances.
    pub class_members: HashSet<String>,
    /// The values each of those classes' companion objects holds, read as `C["Class.name"]`.
    pub companions: HashMap<String, HashSet<String>>,
    /// The functions each of those classes' companion objects has, called on `D.Class`.
    pub companion_functions: HashMap<String, HashSet<String>>,
    /// `typealias ChaptersListDto = List<ChapterDto>`: what each alias stands for.
    pub type_aliases: HashMap<String, Type>,
    /// The source's functions with reified type parameters (`inline fun <reified T> …`): how
    /// many, which a call passes along with its arguments.
    pub reified: HashMap<String, usize>,
    /// The source's own classes (`ReadRealm`, its theme's), whose companions' values are its
    /// constants (`ReadRealm.IMAGE_URL`).
    pub source_classes: HashSet<String>,
    /// Each declared enum's entries, by the enum's name.
    pub enum_entries: HashMap<String, HashSet<String>>,
    /// Fields of the declared classes that are lists wherever a class declares them, for
    /// `dto.titles.orEmpty()`.
    pub list_fields: HashSet<String>,
    /// Constants that are whole numbers (`const val GOLDEN = 0x9E3779B9L`), by the key they're
    /// read by (`C.name`, or a companion's `Class.name`).
    pub constant_numbers: HashMap<String, Num>,
    /// Functions declared to give an `Int` or a `Long` (`fun fnv1a(bytes: ByteArray): Long`), by
    /// name; `None` when overloads of the name disagree.
    pub function_numbers: HashMap<String, Option<Num>>,
    /// Functions that give a sequence (`fun countUp() = sequence { … }`, `: Sequence<T>`), which
    /// a `for` loop reads as it goes.
    pub sequence_functions: HashSet<String>,
    /// Methods of the declared classes whose last parameter is a function with a receiver
    /// (`fun getCollection(path: String, query: CollectionQuery.() -> Unit)`): a trailing lambda
    /// for one reads that receiver implicitly.
    pub receiver_lambda_methods: HashSet<String>,
}

/// What a whole number is in Kotlin, as far as the converter can tell from the code: an `Int`
/// wraps at 32 bits (`hash * 31`, `(hash shl 5) - hash`), a `Long` at 64. JavaScript's numbers
/// are neither, so arithmetic the converter knows the kind of is written to wrap as Kotlin's
/// does, and a `Long` product is worked out exactly (`(hash * 16777619L) and 0xFFFFFFFFL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Num {
    Int,
    Long,
}

impl Num {
    /// A declared type's (`Int`, `Long?`).
    pub fn of_type(ty: &Type) -> Option<Num> {
        match ty.name.as_str() {
            "Int" | "kotlin.Int" => Some(Num::Int),
            "Long" | "kotlin.Long" => Some(Num::Long),
            _ => None,
        }
    }

    /// A literal's (`31`, `-1`, `16777619L`, `0xFFFFFFFF`, which doesn't fit an `Int`).
    pub fn of_literal(expr: &Expr) -> Option<Num> {
        match expr {
            Expr::Int(value) => Some(if i32::try_from(*value).is_ok() {
                Num::Int
            } else {
                Num::Long
            }),
            Expr::Long(_) => Some(Num::Long),
            Expr::Unary { op: "-", expr } => Num::of_literal(expr),
            _ => None,
        }
    }

    /// A property's, from its declared type or its literal value.
    pub fn of_prop(prop: &PropDecl) -> Option<Num> {
        match &prop.ty {
            Some(ty) => Num::of_type(ty),
            None => prop.value.as_ref().and_then(Num::of_literal),
        }
    }

    /// What arithmetic on the two gives: a `Long` with a `Long`, else an `Int`.
    fn with(self, other: Num) -> Num {
        if self == Num::Long || other == Num::Long {
            Num::Long
        } else {
            Num::Int
        }
    }
}

/// The operators a `Long`'s exact arithmetic is worked out with (as JavaScript's `BigInt`).
const EXACT_LONG_OPS: [&str; 11] = [
    "+", "-", "*", "/", "%", "and", "or", "xor", "shl", "shr", "ushr",
];

impl Context {
    /// A type with its aliases replaced by what they stand for.
    pub fn unaliased(&self, ty: &Type) -> Type {
        let short = ty.name.rsplit('.').next().unwrap_or(&ty.name);
        match self.type_aliases.get(short) {
            Some(target) if ty.args.is_empty() => self.unaliased(target),
            _ => Type {
                args: ty.args.iter().map(|arg| self.unaliased(arg)).collect(),
                ..ty.clone()
            },
        }
    }
}

/// A companion value's key on `C`.
pub fn companion_key(class: &str, name: &str) -> String {
    format!("{class}.{name}")
}

/// Method names the runtime has, on some kind of value (`mihon/kotlin.js` keeps the same list).
#[rustfmt::skip]
pub const METHODS: &[&str] = &[
    // Any value
    "toString", "equals", "hashCode", "let", "also", "apply", "run", "takeIf", "takeUnless", "use",
    // Strings
    "substringAfter", "substringBefore", "substringAfterLast", "substringBeforeLast",
    "removePrefix", "removeSuffix", "removeSurrounding", "trim", "trimStart", "trimEnd", "split",
    "lowercase", "uppercase", "contains", "startsWith", "endsWith", "replace", "replaceFirst",
    "isEmpty", "isNotEmpty", "isBlank", "isNotBlank", "isNullOrBlank", "isNullOrEmpty", "ifBlank",
    "ifEmpty", "orEmpty", "toInt", "toIntOrNull", "toLong", "toLongOrNull", "toFloat",
    "toFloatOrNull", "toDouble", "toDoubleOrNull", "toBoolean", "substring", "take", "drop",
    "takeLast", "dropLast", "indexOf", "lastIndexOf", "padStart", "padEnd", "repeat", "lines",
    "matches", "toRegex", "format", "capitalize", "replaceFirstChar", "toHttpUrl",
    "toHttpUrlOrNull", "asJsoup", "parseAs", "trimIndent", "trimMargin", "titlecase", "toCharArray",
    "chunked", "count", "isDigit", "isLetter", "isLetterOrDigit", "isWhitespace", "isUpperCase",
    "isLowerCase", "digitToInt", "code", "toRequestBody", "toMediaType", "toMediaTypeOrNull",
    "encodeToByteArray", "decodeToString", "compareTo", "plus", "minus", "times", "div", "rem",
    "not", "and", "or", "coerceAtLeast", "coerceAtMost", "coerceIn", "toUInt", "roundToInt",
    "roundToLong", "toLocaleString", "reversed", "first", "last", "firstOrNull", "lastOrNull",
    "single", "singleOrNull", "get", "getOrNull", "getOrElse", "filter", "filterNot", "map",
    "mapNotNull", "any", "all", "none", "forEach", "forEachIndexed", "sumOf", "trimEnd",
    // Collections
    "mapIndexed", "mapIndexedNotNull", "filterNotNull", "filterIndexed", "filterIsInstance",
    "onEach", "find", "findLast", "joinToString", "takeWhile", "dropWhile", "distinct",
    "distinctBy", "sorted", "sortedBy", "sortedByDescending", "sortedDescending", "sortedWith",
    "asReversed", "flatten", "flatMap", "groupBy", "associate", "associateBy", "associateWith",
    "toList", "toMutableList", "toSet", "toMutableSet", "toTypedArray", "toMap", "toMutableMap",
    "sum", "maxOf", "minOf", "maxByOrNull", "minByOrNull", "maxOrNull", "minOrNull", "max", "min",
    "zip", "withIndex", "partition", "add", "addAll", "remove", "removeAt", "removeAll", "removeIf",
    "clear", "set", "subList", "firstNotNullOfOrNull", "firstNotNullOf", "indexOfFirst",
    "indexOfLast", "reduce", "fold", "average", "elementAt", "elementAtOrNull", "containsKey",
    "containsValue", "getOrDefault", "put", "putAll", "putIfAbsent", "getOrPut", "entries", "keys",
    "values", "component1", "component2", "component3", "plusElement", "shuffled", "random",
    "slice", "ifEmpty", "isNullOrEmpty", "orEmpty", "asSequence", "asIterable", "toSortedMap",
    "mapKeys", "mapValues", "filterKeys", "filterValues", "windowed", "lastIndexOf", "copy",
    "invoke",
    // Jsoup
    "select", "selectFirst", "selectLast", "attr", "absUrl", "hasAttr", "text", "ownText",
    "wholeText", "html", "outerHtml", "data", "hasClass", "className", "classNames", "id",
    "tagName", "nodeName", "parent", "parents", "children", "child", "nextElementSibling",
    "previousElementSibling", "nextElementSiblings", "siblingElements", "eachText", "eachAttr",
    "location", "body", "head", "title", "is", "not", "attrOrNull", "textOrNull", "ownTextOrNull",
    "hasText", "attributes", "selectFirstOrNull", "firstElementChild", "lastElementChild",
    "elementSiblingIndex", "baseUri",
    // OkHttp, Keiyoushi's helpers, Mihon's models
    "newBuilder", "build", "addQueryParameter", "addEncodedQueryParameter", "setQueryParameter",
    "setEncodedQueryParameter", "removeAllQueryParameters", "addPathSegment", "addPathSegments",
    "addEncodedPathSegment", "addEncodedPathSegments", "setPathSegment", "fragment",
    "queryParameter", "queryParameterValues", "resolve", "header", "headers", "addHeader",
    "removeHeader", "url", "method", "post", "put", "delete", "cacheControl", "fetch", "from",
    "newCall", "execute", "await", "awaitSuccess", "string", "bytes", "close", "body",
    "isSuccessful", "code", "request", "add", "setUrlWithoutDomain", "create", "headersBuilder",
    "getMangaUrl", "getChapterUrl", "tryParse", "parse", "time", "decodeFromString",
    "encodeToString", "decodeFromJsonElement", "jsonObject", "jsonArray", "jsonPrimitive",
    "content", "contentOrNull", "int", "intOrNull", "long", "longOrNull", "double", "doubleOrNull",
    "float", "boolean", "booleanOrNull", "jsonNull", "toJsonString", "toJsonElement",
    "parseToJsonElement", "find", "findAll", "groupValues", "value", "destructured", "range",
    "containsMatchIn", "matchEntire", "groups", "toPattern", "setTime", "getTime", "addHeaderLine",
    "getString", "getBoolean", "getInt", "edit", "putString", "putBoolean", "commit", "apply",
    "use", "head", "rateLimit", "rateLimitHost", "connectTimeout", "readTimeout", "writeTimeout",
    "callTimeout", "followRedirects", "append", "appendLine", "insert", "setLength", "getGenres",
    "toMultimap", "addEncoded", "putJsonObject", "putJsonArray", "getStringSet",
    "firstInstanceOrNull", "isSuccess", "isFailure", "getOrThrow", "getOrDefault",
    "exceptionOrNull", "contentType", "removeHeader", "enqueue", "encodedPath", "port", "host",
    "scheme", "query", "headersOf", "of", "forLanguageTag", "ofPattern", "currentTimeMillis",
    "values", "valueOf", "ordinal", "unaryMinus", "toChar", "tryParseDate", "tryParseDateTime",
    "firstInstance", "mapTo", "mapNotNullTo", "filterTo", "isIncluded", "isExcluded", "isIgnored",
    "toEpochMilliseconds", "toEpochMilli", "getLong", "putInt", "putLong", "putStringSet",
    "fromEpochMilliseconds", "ofEpochMilli", "ofEpochSecond", "now",
    // Coroutines, calendars and zones
    "awaitAll", "withLock", "withPermit", "lock", "unlock", "acquire", "release", "withZone",
    "withLocale", "setTimeZone", "getTimeZone", "systemDefault", "ofHours", "getDefault", "getID",
    "getInstance", "clone", "before", "after", "getTimeInMillis", "setTimeInMillis", "randomUUID",
    // java.time
    "atStartOfDay", "atZone", "toInstant", "toLocalDate", "toEpochSecond", "toEpochDay",
    "minusDays", "plusDays", "minusWeeks", "plusWeeks", "minusMonths", "plusMonths", "minusYears",
    "plusYears", "minusHours", "plusHours", "minusMinutes", "plusMinutes", "minusSeconds",
    "plusSeconds", "getYear", "getMonthValue", "getDayOfMonth", "isBefore", "isAfter", "ofEpochDay",
    "atTime", "atDay", "atEndOfMonth", "lengthOfMonth", "isLeapYear", "withDayOfMonth",
    "withMonth", "withYear", "getHour", "getMinute", "getSecond", "getDayOfYear", "dayOfWeek",
    "weekOfMonth", "weekOfYear", "weekOfWeekBasedYear", "getFirstDayOfWeek",
    // org.json's `JSONObject`/`JSONArray`, `BigInteger`, compressing streams
    "opt", "optString", "optInt", "optLong", "optDouble", "optBoolean", "optJSONObject",
    "optJSONArray", "getJSONObject", "getJSONArray", "isNull", "length", "names", "has",
    "getDouble", "modPow", "modInverse", "pow", "gcd", "shiftLeft", "shiftRight", "testBit",
    "setBit", "bitLength", "bitCount", "negate", "abs", "intValue", "longValue", "doubleValue",
    "add", "subtract", "multiply", "divide", "remainder", "andNot", "toBigIntegerOrNull",
    "finish",
    "unescapeEntities", "getProperty", "getValue",
    // Durations and time units (`5.minutes`, `TimeUnit.MILLISECONDS.toDays(ms)`)
    "toDuration", "isNegative", "isPositive", "isInfinite", "isFinite", "toNanos", "toMicros",
    "toMillis", "toSeconds", "getSeconds", "toMinutes", "toHours", "toDays", "convert",
    // Bits as functions (`x.shl(3)`, `x.inv()`)
    "shl", "shr", "ushr", "inv",
    // `LocalDate.isEqual`, Kotlin's `TimeMark`s (`markNow() + 30.seconds`), a sequence
    // builder's `yield` and `yieldAll`
    "isEqual", "hasPassedNow", "hasNotPassedNow", "yield", "yieldAll", "provideDelegate",
    // Android's `StaticLayout` (`StaticLayout.Builder.obtain(…)…build()`, `layout.draw(canvas)`)
    "obtain", "setAlignment", "setLineSpacing", "setIncludePad", "setHyphenationFrequency",
    "setMaxLines", "setEllipsize", "setBreakStrategy", "setTextDirection", "setJustificationMode",
    "setEllipsizedWidth", "setUseLineSpacingFromFallbacks", "draw", "getLineCount",
    "getLineWidth", "getLineTop", "getLineBottom",
    // Interceptors, cookies, more Jsoup
    "proceed", "intercept", "addInterceptor", "addNetworkInterceptor", "toResponseBody",
    "peekBody", "followSslRedirects", "retryOnConnectionFailure", "cookieJar", "interceptors",
    "message", "readTimeoutMillis", "connectTimeoutMillis", "writeTimeoutMillis", "addCookie",
    "joinTo", "closest", "nextSibling", "previousSibling", "isCanceled", "call",
    // Ordering, JSON bodies, date builders
    "sortWith", "sortBy", "sortByDescending", "thenBy", "thenByDescending", "toJsonRequestBody",
    "getStringOrNull", "getIntOrNull", "getLongOrNull", "getBooleanOrNull", "getVersionName",
    "getVersionCode", "truncatedTo", "appendPattern", "appendLiteral", "appendValue",
    "appendFraction", "appendText", "withoutPadding", "parseCaseInsensitive",
    "parseCaseSensitive", "parseLenient", "parseStrict", "optionalStart", "optionalEnd",
    "parseDefaulting", "toFormatter", "maxOfOrNull", "minOfOrNull", "maxBy", "minBy", "closeQuietly",
    "toByte", "toShort", "toByteArray", "toIntArray",
    "parseGraphQLAs",
    // Next.js pages' data
    "extractNextJs", "extractNextJsRsc",
    // Results
    "onSuccess", "onFailure", "mapCatching", "recover", "recoverCatching",
    // StringBuilder
    "deleteCharAt", "deleteRange", "setCharAt", "reverse", "parseOrNull",
    // Text nodes
    "textNodes", "getWholeText",
    // Bytes, crypto, Markdown descriptions, and the rest of Keiyoushi's helpers
    "decodeHex", "md5", "sha1", "sha256", "sha512", "hmacSha256", "cryptoJsDecrypt",
    "decodeBase64", "copyOfRange", "copyOf", "reversedArray", "sliceArray", "contentEquals",
    "toHexString", "aesDecrypt", "aesEncrypt", "xor", "xorInPlace", "doFinal", "init", "digest",
    "update", "reset", "generateSecret", "getEncoded", "encoded", "getIV", "encode",
    "nextBytes", "nextInt", "nextLong", "nextDouble", "nextFloat", "nextBoolean", "normalize",
    "unescape", "escape", "forName", "defaultCharset", "tryParseZonedDateTime", "rating", "info",
    "description", "markdown", "field", "altTitles", "links", "toMarkdown", "escapeMarkdown",
    "printStackTrace", "ownerDocument", "createElement", "launch", "cancel", "join",
    "getStringSet", "replaceWith", "prepend", "before", "after", "appendText", "prependText",
    "empty", "setType", "addFormDataPart", "addPart", "Builder", "findAnyOf", "topPrivateDomain",
    "encodedQuery", "removePathSegment", "containsAll", "removeRange", "evaluate",
    // Cookies
    "loadForRequest", "saveFromResponse", "getCookie", "setCookie", "flush", "parseAll",
    "hostOnlyDomain", "expiresAt", "secure", "httpOnly", "name", "domain", "path", "hasCookies",
    "setAcceptCookie", "setAcceptThirdPartyCookies", "removeAllCookies", "removeSessionCookies",
    // Pictures: `Bitmap`, `BitmapFactory`, `Canvas`, `Matrix`, `Rect`, `Paint`, `Color`
    "createBitmap", "createScaledBitmap", "decodeStream", "decodeByteArray", "Options",
    "compress", "recycle", "getPixels", "setPixels", "getPixel", "setPixel", "eraseColor",
    "getWidth", "getHeight", "getConfig", "getByteCount", "getRowBytes", "hasAlpha",
    "setHasAlpha", "setDensity", "drawBitmap", "drawColor", "drawARGB", "drawRGB", "drawPaint",
    "drawRect", "save", "restore", "restoreToCount", "getSaveCount", "translate", "scale",
    "rotate", "concat", "setMatrix", "clipRect", "setBitmap", "isIdentity", "setTranslate",
    "setScale", "setRotate", "preTranslate", "preScale", "preRotate", "preConcat",
    "postTranslate", "postScale", "postRotate", "postConcat", "width", "height", "centerX",
    "centerY", "exactCenterX", "exactCenterY", "setEmpty", "offset", "offsetTo", "inset",
    "intersect", "union", "setColor", "setAlpha", "setStyle", "setTextSize", "setTypeface",
    "setAntiAlias", "setFilterBitmap", "setDither", "setXfermode", "setColorFilter", "rgb",
    "argb", "alpha", "red", "green", "blue", "parseColor", "defaultFromStyle", "decode",
    "createUrl",
    // Protobuf: Keiyoushi's readers and writers over kotlinx.serialization's
    "parseAsProto", "decodeProto", "decodeProtoBase64", "toRequestBodyProto", "encodeProto",
    "encodeProtoBase64", "aesDecryptWithPrependedIv", "cipherSource",
    // Lists and Keiyoushi's byte helpers
    "foldIndexed", "addLast", "addFirst", "readIntLittleEndian", "readIntBigEndian",
    "writeIntLittleEndian", "writeIntBigEndian", "getArray", "getArrayOrNull", "getObject",
    "getObjectOrNull",
    // Bodies, streams, okio's `Buffer`, `ByteBuffer`
    "byteStream", "source", "byteString", "charStream", "contentLength", "asResponseBody",
    "write", "writeAll", "writeUtf8", "writeString", "writeByte", "writeShort", "writeShortLe",
    "writeInt", "writeIntLe", "writeLong", "writeLongLe", "readByteArray", "readByteString",
    "readUtf8", "readString", "readUtf8Line", "readByte", "readShort", "readShortLe", "readInt",
    "readIntLe", "readLong", "readLongLe", "readFully", "read", "skip", "exhausted", "require",
    "request", "peek", "snapshot", "outputStream", "inputStream", "byteInputStream", "buffer",
    "emit", "flush", "readBytes", "readAllBytes", "readNBytes", "available", "copyTo",
    "buffered", "reader", "bufferedReader", "readText", "readLine", "readLines",
    "lineSequence", "writeTo", "utf8", "base64", "hex", "wrap", "allocate", "allocateDirect",
    "order", "getShort", "getInt", "getFloat", "getDouble", "putShort", "putInt", "putLong",
    "array", "arrayOffset", "position", "limit", "capacity", "remaining", "hasRemaining",
    "rewind", "flip", "nativeOrder",
    // Comparators, lists' ends and what they keep, sequences and iterators
    "compare", "removeLast", "removeFirst", "removeLastOrNull", "removeFirstOrNull", "retainAll",
    "takeLastWhile", "dropLastWhile", "onEachIndexed", "mapIndexedTo", "flatMapIndexed",
    "flatMapTo", "filterNotNullTo", "groupingBy", "eachCount", "iterator", "hasNext", "next",
    "constrainOnce", "setStrength", "setDecomposition", "getCollationKey",
    // Strings, characters and numbers
    "prependIndent", "replaceRange", "replaceAfter", "replaceAfterLast", "replaceBefore",
    "replaceBeforeLast", "uppercaseChar", "lowercaseChar", "titlecaseChar", "codePointAt",
    "encodeUtf8", "decodeHexToByteString", "toBigDecimal", "toBigInteger", "toULong", "toUByte",
    "toUShort", "mod", "multipliedBy", "separator",
    // Bytes: Keiyoushi's readers, RC4, okio's `ByteString` and `rangeEquals`
    "readUShortLittleEndian", "readUShortBigEndian", "readUIntLittleEndian",
    "readLongLittleEndian", "rc4", "toByteString", "rangeEquals",
    // Jsoup
    "previousElementSiblings", "getElementsByTag", "getElementsByClass", "getElementById",
    "wholeOwnText", "childNodes", "childNodeSize", "val", "setBaseUri",
    // OkHttp's tags and client settings Raygun's own client has
    "tag", "cache", "dns", "protocols", "hostnameVerifier", "sslSocketFactory", "connectionPool",
    "networkInterceptors",
    // Deferred values, caches, addresses
    "complete", "completeExceptionally", "evictAll", "snapshot", "getQueryParameter",
    "getQueryParameterNames", "buildUpon", "toURL", "toURI", "toExternalForm",
    // GCM's data, Android's `Uri.Builder`, settings widgets, Jsoup and the rest
    "updateAAD", "appendQueryParameter", "appendPath", "appendEncodedPath", "authority",
    "setDefaultValue", "setOnPreferenceChangeListener", "setOnPreferenceClickListener",
    "setOnBindEditTextListener", "addPreference", "parseBodyFragment", "nextDouble", "shuffle",
    "toJavaInstant", "toKotlinInstant", "stripTrailingZeros", "toPlainString", "noCache",
    "noStore", "maxStale", "maxAge", "onlyIfCached", "protocol", "subSequence", "inflate",
    "copyInto", "insertChildren", "atYear", "getResource", "getResourceAsStream", "openStream",
    "serializer", "getDate", "getDayOfWeek", "getDisplayName", "getDisplayLanguage",
    "toLanguageTag", "toBigDecimalOrNull", "randomOrNull", "minWith", "maxWith", "runningFold",
    "siblingIndex", "normalName", "getName", "languageDisplayName", "downTo", "contentToString",
    "appendCodePoint", "signum", "then", "thenComparator", "thenDescending", "elapsedNow",
    "markNow", "isISOControl", "fromEpochSeconds", "incrementAndGet", "getAndIncrement",
    "decrementAndGet", "getAndDecrement", "addAndGet", "getAndAdd", "getAndSet", "compareAndSet",
    "tryLock", "readLock", "writeLock", "isHeldByCurrentThread", "setScale", "simpleName",
    "fixedLength", "cancelChildren", "availableProcessors", "maxMemory", "totalMemory",
    "freeMemory", "addJsonObject", "addJsonArray", "dispatcher", "withDefault", "exists",
    "mkdirs", "mkdir", "createNewFile", "deleteRecursively", "lastModified", "setLastModified",
    "readBytes", "writeBytes", "writeText", "appendText", "listFiles", "renameTo",
    "createTempFile", "isDirectory", "isEqual", "getOffset", "getRawOffset", "freeze",
    "cloneAsThawed", "toHashSet", "asList", "getElementsContainingText",
];

fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

/// A JavaScript name for a Kotlin local.
pub fn local(name: &str) -> String {
    let mut out = String::from("$");
    for char in name.chars() {
        if char.is_ascii_alphanumeric() || char == '_' {
            out.push(char);
        } else {
            out.push('_');
        }
    }
    out
}

/// How a block's last expression is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tail {
    /// Returned (a function's expression body, a lambda, a branch of an expression).
    Return,
    /// Dropped.
    Discard,
}

pub struct Gen<'a> {
    cx: &'a Context,
    /// Locals in scope, innermost last: each Kotlin name's JavaScript name.
    scopes: Vec<HashMap<String, String>>,
    /// Implicit receivers, innermost last (their JavaScript names).
    receivers: Vec<String>,
    /// The labels each receiver answers to in `this@label`: its function's name, its class's.
    receiver_labels: Vec<Vec<String>>,
    /// Labels of the lambdas being made, innermost last.
    labels: Vec<String>,
    /// How deep in lambdas and expression functions the code is: a plain `return` there leaves
    /// the Kotlin function, which the runtime does with an exception.
    depth: usize,
    /// A plain `return` was made into `KT.ret`: the function catches it.
    pub catches_return: bool,
    next: u32,
    /// The declared class whose code this is: its companion's values are in scope.
    pub class_scope: Option<String>,
    /// The Kotlin function being written (`passCheck`), which tells a web view passing a
    /// browser check from one reading a page.
    function_name: String,
    /// Its declared return type: what `return text.parseAs()` decodes into.
    return_type: Option<Type>,
    /// Its reified type parameters (`T`) and the JavaScript names their types are in.
    reified_types: Vec<(String, String)>,
    /// Local functions with a receiver (`fun Builder.addParam(…)` in a body), by JavaScript name:
    /// their receiver is passed first.
    local_extensions: HashSet<String>,
    /// The lambda depth of each loop being written, innermost last.
    loops: Vec<Loop>,
    /// The loops a `break` or `continue` leaves from a lambda or an expression (by id): their
    /// bodies catch the jump (`KT.Jump`), as JavaScript can't break out of a function.
    jumped: HashSet<u32>,
    /// Classes the converter writes itself instead (a theme's image interceptor), by name.
    pub replaced: HashMap<String, String>,
    /// Parameters declared with a type that isn't a function's, by JavaScript name: a call by
    /// their name is the function they shadow (`fetchChapters(…)` beside `fetchChapters: Boolean`).
    plain: HashSet<String>,
    /// Locals declared with `val`, by JavaScript name: `+=` on one adds to it in place.
    vals: HashSet<String>,
    /// The imports of the file being written (name as used, full name).
    pub imports: Vec<(String, String)>,
    /// Parameters that are functions with a receiver, by JavaScript name (`builder.params()`).
    receiver_params: HashSet<String>,
    /// The type a decoder without a type argument decodes into: the declared type of what it
    /// gives (`fun parse(): Dto = text.parseAs()`, `val dto: Dto = text.parseAs()`).
    expected: Option<Type>,
    /// Locals and parameters that are whole numbers of a kind Kotlin fixes (`var hash = 0` is an
    /// `Int`), by JavaScript name; every declaration clears its name first.
    numbers: HashMap<String, Num>,
}

/// Calls that decode into the type they're given.
const DECODER_CALLS: [&str; 7] = [
    "parseAs",
    "decodeFromString",
    "decodeFromJsonElement",
    "parseAsProto",
    "decodeProto",
    "decodeProtoBase64",
    "decodeFromByteArray",
];

/// Whether `expr` is a decoder called without a type argument (`text.parseAs()`).
fn untyped_decoder(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call { callee, type_args, .. }
            if type_args.is_empty()
                && matches!(&**callee, Expr::Member { name, .. } if DECODER_CALLS.contains(&name.as_str()))
    )
}

/// Functions whose lambda has no parameter (`ifEmpty { … }`, `require(…) { "message" }`).
const NO_PARAMETER_LAMBDAS: [&str; 16] = [
    "ifEmpty",
    "ifBlank",
    "getOrPut",
    "lazy",
    "run",
    "runCatching",
    "synchronized",
    "measureTimeMillis",
    "withLock",
    "withPermit",
    "require",
    "requireNotNull",
    "check",
    "checkNotNull",
    "async",
    "coroutineScope",
];

/// Packages whose classes a file's import means instead of a source's own class of the same name
/// (`Page` from Mihon's models beside a DTO called `Page`).
const LIBRARY_PACKAGES: [&str; 11] = [
    "eu.kanade.tachiyomi.source.",
    "eu.kanade.tachiyomi.network.",
    "okhttp3.",
    "org.jsoup.",
    "kotlinx.",
    "kotlin.",
    "java.",
    "keiyoushi.",
    "android.",
    "rx.",
    "uy.kohesive.",
];

/// The first segments of names written with their package (`java.net.URI(…)`,
/// `eu.kanade.tachiyomi.source.model.Filter.Header(…)`), which the converter reads without it.
const PACKAGE_ROOTS: [&str; 14] = [
    "java",
    "javax",
    "kotlin",
    "kotlinx",
    "eu",
    "org",
    "okhttp3",
    "okio",
    "android",
    "androidx",
    "keiyoushi",
    "uy",
    "rx",
    "com",
];

/// A loop being written: how deep in lambdas it is, its label, and an id jumps name it by.
#[derive(Debug, Clone)]
struct Loop {
    depth: usize,
    label: Option<String>,
    id: u32,
}

fn unsupported(what: impl std::fmt::Display) -> String {
    format!("its code uses {what}, which Raygun's converter doesn't read yet")
}

impl<'a> Gen<'a> {
    pub fn new(cx: &'a Context) -> Self {
        Self {
            cx,
            scopes: vec![HashMap::new()],
            receivers: Vec::new(),
            receiver_labels: Vec::new(),
            labels: Vec::new(),
            depth: 0,
            catches_return: false,
            next: 0,
            class_scope: None,
            function_name: String::new(),
            return_type: None,
            reified_types: Vec::new(),
            local_extensions: HashSet::new(),
            loops: Vec::new(),
            jumped: HashSet::new(),
            replaced: HashMap::new(),
            plain: HashSet::new(),
            expected: None,
            vals: HashSet::new(),
            imports: Vec::new(),
            receiver_params: HashSet::new(),
            numbers: HashMap::new(),
        }
    }

    /// Starts a loop (with its Kotlin label) and gives its id.
    fn enter_loop(&mut self, label: Option<&String>) -> u32 {
        self.next += 1;
        let id = self.next;
        self.loops.push(Loop {
            depth: self.depth,
            label: label.cloned(),
            id,
        });
        id
    }

    /// A loop's body, catching the jumps made out of lambdas or expressions inside it.
    fn loop_body(&mut self, id: u32, body: String) -> String {
        if self.jumped.remove(&id) {
            format!(
                "try {{ {body} }} catch (e) {{ if (e instanceof KT.Jump && e.loop === {id}) {{ if (e.kind === \"break\") break; continue; }} throw e; }}"
            )
        } else {
            body
        }
    }

    /// The loop a `break` or `continue` (with its label) leaves.
    fn jump_target(&self, label: Option<&String>) -> Result<Loop, String> {
        self.loops
            .iter()
            .rev()
            .find(|found| label.is_none() || found.label.as_ref() == label)
            .cloned()
            .ok_or_else(|| unsupported("break or continue outside a loop"))
    }

    /// A `break` or `continue` as a statement: JavaScript's own when it's in the loop's function
    /// and needs no label it can't see, else a `KT.Jump` the loop's body catches.
    fn jump_statement(&mut self, kind: &str, label: Option<&String>) -> Result<String, String> {
        let target = self.jump_target(label)?;
        if target.depth == self.depth {
            return Ok(match label {
                Some(label) => format!("{kind} {};", local(label)),
                None => format!("{kind};"),
            });
        }
        self.jumped.insert(target.id);
        Ok(format!(
            "throw new KT.Jump({}, {});",
            quote(kind),
            target.id
        ))
    }

    /// A `break` or `continue` where a value goes (`?: continue`): a jump thrown.
    fn jump_expression(&mut self, kind: &str, label: Option<&String>) -> Result<String, String> {
        let target = self.jump_target(label)?;
        self.jumped.insert(target.id);
        Ok(format!("KT.jump({}, {})", quote(kind), target.id))
    }

    /// A type with the source's type aliases replaced.
    pub fn unaliased(&self, ty: &Type) -> Type {
        self.cx.unaliased(ty)
    }

    /// Whether `name` here is a library's class the file imports rather than one of the source's
    /// own classes of that name.
    fn imported_from_library(&self, name: &str) -> bool {
        self.imports.iter().any(|(alias, full)| {
            alias == name
                // Keiyoushi's libraries' code converts with the extension's (`lib:unpacker`).
                && !full.starts_with("keiyoushi.lib.")
                && LIBRARY_PACKAGES
                    .iter()
                    .any(|package| full.starts_with(package))
        })
    }

    /// A class the source declares, as `name` means it here.
    fn declared_class(&self, name: &str) -> bool {
        self.cx.classes.contains(name) && !self.imported_from_library(name)
    }

    /// The class a `typealias` names, when the source declares it
    /// (`typealias LatestQueryVariables = PopularQueryVariables`, made by the alias's name).
    fn aliased_class(&self, name: &str) -> Option<String> {
        if self.declared_class(name) || self.is_local(name) {
            return None;
        }
        let target = self.cx.type_aliases.get(name)?;
        let simple = target.name.rsplit('.').next().unwrap_or(&target.name);
        self.declared_class(simple).then(|| simple.to_owned())
    }

    /// A value of the current class's companion, by name.
    fn companion_value(&self, name: &str) -> Option<String> {
        let class = self.class_scope.as_deref()?;
        self.cx
            .companions
            .get(class)
            .filter(|names| names.contains(name))
            .map(|_| format!("(await C.get({}))", quote(&companion_key(class, name))))
    }

    fn temp(&mut self) -> String {
        self.next += 1;
        format!("$t{}", self.next)
    }

    /// Whether a member call by this name has somewhere to go: the runtime's tables, or a class
    /// the source declares.
    fn known(&self, name: &str) -> bool {
        METHODS.contains(&name)
            || self.cx.class_members.contains(name)
            // The source passed around as a value (`source.toImage(url)` in a DTO).
            || self.cx.functions.contains_key(name)
    }

    /// `java.net.URI`, `eu.kanade.tachiyomi.source.model.Filter.Header`: a name written with its
    /// package, read without it (`URI`, `Filter.Header`).
    fn without_package(&self, expr: &Expr) -> Option<Expr> {
        let mut segments = Vec::new();
        let mut at = expr;
        loop {
            match at {
                Expr::Member {
                    target,
                    name,
                    safe: false,
                } => {
                    segments.push(name.as_str());
                    at = target;
                }
                Expr::Name(root) => {
                    segments.push(root.as_str());
                    break;
                }
                _ => return None,
            }
        }
        segments.reverse();
        let root = segments[0];
        if !PACKAGE_ROOTS.contains(&root)
            || self.is_local(root)
            || self.cx.properties.contains(root)
            || self.cx.constants.contains(root)
        {
            return None;
        }
        let first_type = segments
            .iter()
            .position(|segment| segment.chars().next().is_some_and(char::is_uppercase))?;
        let mut out = Expr::Name(segments[first_type].to_owned());
        for segment in &segments[first_type + 1..] {
            out = Expr::Member {
                target: Box::new(out),
                name: (*segment).to_owned(),
                safe: false,
            };
        }
        Some(out)
    }

    /// A type as written for the runtime: its text, with the reified type parameters in scope
    /// put in when the code runs (`NextData<T>` → `NextData<MangaListDto>`).
    fn type_code(&self, text: &str) -> String {
        let used: Vec<String> = self
            .reified_types
            .iter()
            .filter(|(name, _)| {
                text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .any(|word| word == name)
            })
            .map(|(name, js)| format!("{}: {js}", quote(name)))
            .collect();
        if used.is_empty() {
            quote(text)
        } else {
            format!("KT.typeWith({}, {{{}}})", quote(text), used.join(", "))
        }
    }

    /// Whether an expression's value is a list: a call answering one (`split(…)`, `map { … }`)
    /// or a field the declared classes keep lists in.
    fn gives_a_list(&self, expr: &Expr) -> bool {
        const LIST_RESULTS: [&str; 30] = [
            "split",
            "map",
            "mapNotNull",
            "mapIndexed",
            "mapIndexedNotNull",
            "filterNotNull",
            "select",
            "toList",
            "toMutableList",
            "sortedBy",
            "sortedByDescending",
            "distinctBy",
            "chunked",
            "flatMap",
            "flatten",
            "lines",
            "children",
            "eachText",
            "eachAttr",
            "listOf",
            "emptyList",
            "groupValues",
            "zip",
            "windowed",
            "toTypedArray",
            "filterIsInstance",
            "textNodes",
            "parents",
            "siblingElements",
            "nextElementSiblings",
        ];
        match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Member { name, .. } | Expr::Name(name) => {
                    LIST_RESULTS.contains(&name.as_str())
                }
                _ => false,
            },
            Expr::Member { name, .. } => self.cx.list_fields.contains(name),
            _ => false,
        }
    }

    /// `MangaResponse.Props`: classes the source declares, nested in one another.
    fn declared_path(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Name(name) => self.cx.classes.contains(name) && !self.is_local(name),
            Expr::Member {
                target,
                name,
                safe: false,
            } => self.cx.classes.contains(name) && self.declared_path(target),
            _ => false,
        }
    }

    /// `Filter.Sort`: classes of the runtime's nested in one another, not a value of the source's.
    fn is_type_path(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Name(name) => {
                name.chars().next().is_some_and(char::is_uppercase)
                    && !self.is_local(name)
                    && !self.cx.properties.contains(name)
                    && !self.cx.constants.contains(name)
                    && !self.cx.classes.contains(name)
            }
            Expr::Member {
                target,
                name,
                safe: false,
            } => name.chars().next().is_some_and(char::is_uppercase) && self.is_type_path(target),
            _ => false,
        }
    }

    /// `i++`, `++count`, `this.page--`, `counts[key]++` as a value: the variable moves on, and the
    /// value is the one before it did (postfix) or after (prefix).
    fn increment(&mut self, op: &str, target: &Expr, postfix: bool) -> Result<String, String> {
        if let Expr::Name(name) = target
            && let Some(js) = self.resolve(name)
        {
            return Ok(if postfix {
                format!("({js}{op})")
            } else {
                format!("({op}{js})")
            });
        }
        let step = if op == "++" { "+ 1" } else { "- 1" };
        let (old, new) = (self.temp(), self.temp());
        let (setup, read, write) = match target {
            Expr::Name(name) => {
                let receivers = self.receiver_list();
                (
                    String::new(),
                    format!("await KT.rget({receivers}, {})", quote(name)),
                    format!("await KT.rset({receivers}, {}, {new})", quote(name)),
                )
            }
            Expr::Member { target, name, .. } => {
                let object = self.expr(target)?;
                let holder = self.temp();
                (
                    format!("const {holder} = {object}; "),
                    format!("await KT.p({holder}, {})", quote(name)),
                    format!("await KT.setp({holder}, {}, {new})", quote(name)),
                )
            }
            Expr::Index { target, index } if index.len() == 1 => {
                let object = self.expr(target)?;
                let key = self.expr(&index[0])?;
                let (holder, key_temp) = (self.temp(), self.temp());
                (
                    format!("const {holder} = {object}; const {key_temp} = {key}; "),
                    format!("await KT.m({holder}, \"get\", [{key_temp}])"),
                    format!("await KT.m({holder}, \"set\", [{key_temp}, {new}])"),
                )
            }
            _ => return Err(unsupported(format!("{op} on that"))),
        };
        let result = if postfix { &old } else { &new };
        Ok(format!(
            "(await (async () => {{ {setup}const {old} = {read}; const {new} = {old} {step}; {write}; return {result}; }})())"
        ))
    }

    /// Declares a local and gives its JavaScript name. Kotlin lets a local shadow a parameter or
    /// an outer local, even reading it in its own initializer (`val page = page + 1`), which
    /// JavaScript doesn't: such a local gets a name of its own.
    fn declare(&mut self, name: &str) -> String {
        let js = if self.is_local(name) {
            self.next += 1;
            format!("{}${}", local(name), self.next)
        } else {
            local(name)
        };
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_owned(), js.clone());
        }
        self.numbers.remove(&js);
        js
    }

    /// Declares a local of a known kind of number (or none).
    fn declare_number(&mut self, name: &str, kind: Option<Num>) -> String {
        let js = self.declare(name);
        if let Some(kind) = kind {
            self.numbers.insert(js.clone(), kind);
        }
        js
    }

    /// What kind of whole number an expression is, when Kotlin's types fix it: literals, locals
    /// and parameters of a known kind, constants, `toInt()`/`toLong()`/`hashCode()`, functions
    /// declared to give one, and arithmetic on those.
    fn num(&self, expr: &Expr) -> Option<Num> {
        match expr {
            Expr::Int(_) | Expr::Long(_) => Num::of_literal(expr),
            Expr::Name(name) => match self.resolve(name) {
                Some(js) => self.numbers.get(&js).copied(),
                None => self
                    .class_scope
                    .as_ref()
                    .and_then(|class| {
                        self.cx
                            .constant_numbers
                            .get(&companion_key(class, name))
                            .copied()
                    })
                    .or_else(|| self.cx.constant_numbers.get(name).copied()),
            },
            Expr::NotNull(inner) => self.num(inner),
            Expr::Unary {
                op: "-" | "+",
                expr,
            } => self.num(expr),
            Expr::Call {
                callee,
                lambda: None,
                ..
            } => match callee.as_ref() {
                Expr::Member { target, name, .. } => match name.as_str() {
                    "toInt" | "hashCode" | "roundToInt" | "digitToInt" => Some(Num::Int),
                    "toLong" | "roundToLong" => Some(Num::Long),
                    "inv" => self.num(target),
                    _ => None,
                },
                Expr::Name(name) if !self.is_local(name) => {
                    self.cx.function_numbers.get(name).copied().flatten()
                }
                _ => None,
            },
            Expr::Binary { op, left, right } => self.num_binary(op, left, right),
            _ => None,
        }
    }

    fn num_binary(&self, op: &str, left: &Expr, right: &Expr) -> Option<Num> {
        match op {
            // A shift is its left side's kind, whatever the count is.
            "shl" | "shr" | "ushr" => self.num(left),
            "+" | "-" | "*" | "/" | "%" | "and" | "or" | "xor" => {
                Some(self.num(left)?.with(self.num(right)?))
            }
            _ => None,
        }
    }

    /// Whether this is a sequence that may not end: one a builder makes (`sequence { … }`,
    /// `generateSequence(…)`) or one of the source's functions that give one.
    fn gives_a_sequence(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Name(name) if !self.is_local(name) => {
                    name == "sequence"
                        || name == "generateSequence"
                        || self.cx.sequence_functions.contains(name)
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// The kind of number a loop over this goes through: `0 until n`, `a..b`, `n downTo 0` of
    /// `Int`s (or `Long`s), a list's `indices`.
    fn range_num(&self, iterable: &Expr) -> Option<Num> {
        match iterable {
            Expr::Binary { op, left, right } => match op.as_str() {
                ".." | "..<" | "until" | "downTo" => Some(self.num(left)?.with(self.num(right)?)),
                "step" => self.range_num(left),
                _ => None,
            },
            Expr::Member { name, .. } if name == "indices" => Some(Num::Int),
            _ => None,
        }
    }

    /// Whether a `Long`'s arithmetic has a product or a left shift in it (outside of calls),
    /// which may pass what JavaScript's numbers hold exactly (2^53).
    fn has_long_growth(&self, op: &str, left: &Expr, right: &Expr) -> bool {
        let part = |expr: &Expr| match expr {
            Expr::Binary { op, left, right } => self.has_long_growth(op, left, right),
            _ => false,
        };
        EXACT_LONG_OPS.contains(&op)
            && self.num_binary(op, left, right) == Some(Num::Long)
            && (matches!(op, "*" | "shl") || part(left) || part(right))
    }

    /// A `Long`'s arithmetic as JavaScript's `BigInt`s, wrapping at 64 bits as Kotlin's does;
    /// what isn't part of it is worked out as usual and made a `BigInt` (`KT.big`).
    fn exact_long(&mut self, op: &str, left: &Expr, right: &Expr) -> Result<String, String> {
        let (left, right) = (self.exact_part(left)?, self.exact_part(right)?);
        Ok(match op {
            "+" => format!("BigInt.asIntN(64, {left} + {right})"),
            "-" => format!("BigInt.asIntN(64, {left} - {right})"),
            "*" => format!("BigInt.asIntN(64, {left} * {right})"),
            "/" => format!("({left} / {right})"),
            "%" => format!("({left} % {right})"),
            "and" => format!("({left} & {right})"),
            "or" => format!("({left} | {right})"),
            "xor" => format!("({left} ^ {right})"),
            "shl" => format!("BigInt.asIntN(64, {left} << ({right} & 63n))"),
            "shr" => format!("({left} >> ({right} & 63n))"),
            _ => format!("BigInt.asIntN(64, BigInt.asUintN(64, {left}) >> ({right} & 63n))"),
        })
    }

    fn exact_part(&mut self, expr: &Expr) -> Result<String, String> {
        match expr {
            Expr::Binary { op, left, right }
                if EXACT_LONG_OPS.contains(&op.as_str())
                    && self.num_binary(op, left, right) == Some(Num::Long) =>
            {
                self.exact_long(op, left, right)
            }
            Expr::Int(value) | Expr::Long(value) => Ok(format!("{value}n")),
            _ => Ok(format!("KT.big({})", self.expr(expr)?)),
        }
    }

    /// A local's JavaScript name, when the name is a local here.
    fn resolve(&self, name: &str) -> Option<String> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn is_local(&self, name: &str) -> bool {
        self.resolve(name).is_some()
    }

    fn scoped<T>(&mut self, work: impl FnOnce(&mut Self) -> T) -> T {
        self.scopes.push(HashMap::new());
        let result = work(self);
        self.scopes.pop();
        result
    }

    fn push_receiver<'l>(&mut self, name: &str, labels: impl IntoIterator<Item = Option<&'l str>>) {
        self.receivers.push(name.to_owned());
        self.receiver_labels
            .push(labels.into_iter().flatten().map(str::to_owned).collect());
    }

    fn pop_receiver(&mut self) {
        self.receivers.pop();
        self.receiver_labels.pop();
    }

    /// `this@label`: the receiver answering to it, else the source.
    fn labelled_this(&self, label: &str) -> String {
        self.receivers
            .iter()
            .zip(&self.receiver_labels)
            .rev()
            .find(|(_, labels)| labels.iter().any(|known| known == label))
            .map_or_else(|| "self".to_owned(), |(receiver, _)| receiver.clone())
    }

    /// The receivers in scope, innermost first, then the source: `[$r2, $r1, self]`.
    fn receiver_list(&self) -> String {
        let mut list: Vec<&str> = self.receivers.iter().rev().map(String::as_str).collect();
        list.push("self");
        format!("[{}]", list.join(", "))
    }

    // ---- Functions ----

    /// A function's JavaScript: `async ($a, $b = 2) => { … }`; a receiver comes first as `$this`.
    pub fn function(&mut self, fun: &FunDecl) -> Result<String, String> {
        let body = fun
            .body
            .as_ref()
            .ok_or_else(|| format!("{} has no body", fun.name))?;
        // The function's own state; the outer one comes back whatever happens inside.
        let saved = (
            self.depth,
            self.catches_return,
            std::mem::take(&mut self.labels),
            self.receivers.len(),
        );
        let outer_loops = std::mem::take(&mut self.loops);
        let outer_name = std::mem::replace(&mut self.function_name, fun.name.clone());
        let outer_return = std::mem::replace(&mut self.return_type, fun.ret.clone());
        // A function's reified type parameters: the types its call passes, taken first.
        let reified: Vec<(String, String)> = fun
            .modifiers
            .iter()
            .filter_map(|modifier| modifier.strip_prefix("reified:"))
            .map(|name| (name.to_owned(), format!("$type${name}")))
            .collect();
        let outer_reified = if reified.is_empty() {
            None
        } else {
            Some(std::mem::replace(&mut self.reified_types, reified.clone()))
        };
        self.depth = 0;
        self.catches_return = false;
        let result = self.scoped(|writer| {
            let mut params = Vec::new();
            // Defaults are worked out in the body, in order: they may await, which a parameter
            // list can't.
            let mut defaults = Vec::new();
            if let Some(receiver) = &fun.receiver {
                // An outer function's receiver stays reachable (`this@Outer`).
                let name = if writer.receivers.is_empty() {
                    "$this".to_owned()
                } else {
                    writer.temp()
                };
                let class = receiver.name.rsplit('.').next().unwrap_or(&receiver.name);
                params.push(name.clone());
                writer.push_receiver(&name, [Some(fun.name.as_str()), Some(class)]);
            }
            // Context parameters (`context(source: HttpSource)`) are the source running.
            for modifier in &fun.modifiers {
                if let Some(context) = modifier.strip_prefix("context:") {
                    let name = writer.declare(context);
                    defaults.push(format!("const {name} = self;"));
                }
            }
            for param in &fun.params {
                let kind = if param.vararg {
                    None
                } else {
                    Num::of_type(&param.ty)
                };
                let name = writer.declare_number(&param.name, kind);
                writer.vals.remove(&name);
                if !param.ty.name.is_empty() && !param.ty.name.starts_with('(') {
                    writer.plain.insert(name.clone());
                } else {
                    writer.plain.remove(&name);
                }
                if param.ty.name == RECEIVER_FUNCTION {
                    writer.receiver_params.insert(name.clone());
                } else {
                    writer.receiver_params.remove(&name);
                }
                if let Some(default) = &param.default {
                    defaults.push(format!(
                        "if ({name} === undefined) {name} = {};",
                        writer.expr(default)?
                    ));
                }
                // A vararg is JavaScript's rest parameter when it's the last; before others it's
                // a list the call makes (`transpile::param_names` marks it `*`).
                let last = std::ptr::eq(param, fun.params.last().expect("a parameter"));
                params.push(if param.vararg && last {
                    format!("...{name}")
                } else if param.vararg {
                    format!("{name} = []")
                } else {
                    name
                });
            }
            let code = match body {
                Body::Expr(expr) => {
                    if untyped_decoder(expr) {
                        writer.expected = fun.ret.clone();
                    }
                    format!("return {};", writer.expr(expr)?)
                }
                Body::Block(block) => writer.block(block, Tail::Discard)?,
            };
            let code = if defaults.is_empty() {
                code
            } else {
                format!("{} {code}", defaults.join(" "))
            };
            // The types a call passed, read before anything waits.
            let code = if reified.is_empty() {
                code
            } else {
                let names: Vec<&str> = reified.iter().map(|(_, js)| js.as_str()).collect();
                format!(
                    "const [{}] = KT.takeTypes({}); {code}",
                    names.join(", "),
                    names.len()
                )
            };
            let code = if writer.catches_return {
                format!("try {{ {code} }} catch (e) {{ if (e instanceof KT.Return) return e.value; throw e; }}")
            } else {
                code
            };
            Ok(format!("async ({}) => {{ {code} }}", params.join(", ")))
        });
        let (depth, catches_return, labels, receivers) = saved;
        self.depth = depth;
        self.catches_return = catches_return;
        self.labels = labels;
        self.loops = outer_loops;
        self.function_name = outer_name;
        self.return_type = outer_return;
        if let Some(outer) = outer_reified {
            self.reified_types = outer;
        }
        self.receivers.truncate(receivers);
        self.receiver_labels.truncate(receivers);
        result
    }

    // ---- Statements ----

    pub fn block(&mut self, block: &Block, tail: Tail) -> Result<String, String> {
        self.scoped(|writer| {
            let mut out = Vec::new();
            for (index, statement) in block.iter().enumerate() {
                let last = index + 1 == block.len();
                out.push(writer.statement(statement, last && tail == Tail::Return)?);
            }
            if tail == Tail::Return && block.is_empty() {
                out.push("return KT.unit;".to_owned());
            }
            Ok(out.join(" "))
        })
    }

    /// A local's initial value: its initializer, or `by lazy { … }`'s.
    fn local_value(
        &mut self,
        value: Option<&Expr>,
        delegate: Option<&Expr>,
    ) -> Result<String, String> {
        Ok(match (value, delegate) {
            (Some(value), _) => self.expr(value)?,
            (
                None,
                Some(Expr::Call {
                    callee,
                    lambda: Some(lambda),
                    ..
                }),
            ) if matches!(callee.as_ref(), Expr::Name(name) if name == "lazy") => {
                self.lambda_call(lambda, "lazy")?
            }
            (None, Some(_)) => return Err(unsupported("a delegated local")),
            (None, None) => "undefined".to_owned(),
        })
    }

    /// A `do { … } while (…)`: the body's own locals are declared before the loop, as Kotlin's
    /// condition sees them (`do { val page = … } while (page.hasNext)`).
    fn do_while(
        &mut self,
        label: Option<&String>,
        condition: &Expr,
        body: &Block,
    ) -> Result<String, String> {
        let id = self.enter_loop(label);
        let result = self.scoped(|writer| -> Result<(String, String, Vec<String>), String> {
            let mut hoisted = Vec::new();
            let mut out = Vec::new();
            for statement in body {
                match statement {
                    Stmt::Val {
                        names,
                        destructured: false,
                        mutable,
                        ty,
                        value,
                        delegate,
                    } if names.len() == 1 => {
                        let kind = match ty {
                            Some(ty) => Num::of_type(ty),
                            None => value.as_ref().and_then(|value| writer.num(value)),
                        };
                        let value = writer.local_value(value.as_ref(), delegate.as_ref())?;
                        writer.next += 1;
                        let js = format!("{}${}", local(&names[0]), writer.next);
                        if let Some(scope) = writer.scopes.last_mut() {
                            scope.insert(names[0].clone(), js.clone());
                        }
                        match kind {
                            Some(kind) => writer.numbers.insert(js.clone(), kind),
                            None => writer.numbers.remove(&js),
                        };
                        writer.plain.remove(&js);
                        if *mutable {
                            writer.vals.remove(&js);
                        } else {
                            writer.vals.insert(js.clone());
                        }
                        out.push(format!("{js} = {value};"));
                        hoisted.push(js);
                    }
                    _ => out.push(writer.statement(statement, false)?),
                }
            }
            let condition = writer.expr(condition)?;
            Ok((out.join(" "), condition, hoisted))
        });
        self.loops.pop();
        let (body, condition, hoisted) = result?;
        let body = self.loop_body(id, body);
        let declared = if hoisted.is_empty() {
            String::new()
        } else {
            format!("let {}; ", hoisted.join(", "))
        };
        let label = label
            .map(|label| format!("{}: ", local(label)))
            .unwrap_or_default();
        Ok(format!(
            "{declared}{label}do {{ {body} }} while (KT.truthy({condition}));"
        ))
    }

    fn statement(&mut self, statement: &Stmt, returned: bool) -> Result<String, String> {
        Ok(match statement {
            Stmt::Val {
                names,
                destructured,
                mutable,
                ty,
                value,
                delegate,
            } => {
                // A `val` without a value (`val x: Int` then set in each branch) is set later.
                let keyword = if *mutable || (value.is_none() && delegate.is_none()) {
                    "let"
                } else {
                    "const"
                };
                // Its kind of number: as declared, else its value's.
                let value_kind = value.as_ref().and_then(|value| self.num(value));
                let kind = match ty {
                    Some(ty) => Num::of_type(ty),
                    None => value_kind,
                };
                let wraps = kind == Some(Num::Int) && value.is_some() && value_kind != kind;
                let value = self.local_value(value.as_ref(), delegate.as_ref())?;
                let value = if wraps {
                    format!("KT.i32({value})")
                } else {
                    value
                };
                // Declared after the value is worked out: an initializer reads what the name
                // meant before.
                let code = if *destructured {
                    let temp = self.temp();
                    let mut parts = Vec::new();
                    for (index, name) in names.iter().enumerate() {
                        if name != "_" {
                            let js = self.declare(name);
                            parts.push(format!("{js} = KT.component({temp}, {index})"));
                        }
                    }
                    if parts.is_empty() {
                        format!("const {temp} = {value};")
                    } else {
                        format!("const {temp} = {value}; {keyword} {};", parts.join(", "))
                    }
                } else {
                    let js = self.declare_number(&names[0], kind);
                    self.plain.remove(&js);
                    if *mutable {
                        self.vals.remove(&js);
                    } else {
                        self.vals.insert(js.clone());
                    }
                    format!("{keyword} {js} = {value};")
                };
                if returned {
                    format!("{code} return KT.unit;")
                } else {
                    code
                }
            }
            Stmt::Assign { target, op, value } => {
                // A local whole number of a kind the code fixes stays one (`hash = hash * 31 + c`
                // wraps at 32 bits for an `Int`).
                let target_kind = match target {
                    Expr::Name(name) => self
                        .resolve(name)
                        .and_then(|js| self.numbers.get(&js).copied()),
                    _ => None,
                };
                let value_kind = self.num(value);
                let value = self.expr(value)?;
                let combined = |current: String| -> String {
                    match (*op, target_kind) {
                        ("=", Some(Num::Int)) if value_kind != Some(Num::Int) => {
                            format!("KT.i32({value})")
                        }
                        ("=", _) => value.clone(),
                        ("+=", Some(Num::Int)) => format!("(({current} + {value}) | 0)"),
                        ("-=", Some(Num::Int)) => format!("(({current} - {value}) | 0)"),
                        ("*=", Some(Num::Int)) => format!("Math.imul({current}, {value})"),
                        ("*=", Some(Num::Long)) => format!("KT.lmul({current}, {value})"),
                        ("+=", _) => format!("KT.plusAssign({current}, {value})"),
                        ("-=", _) => format!("KT.minusAssign({current}, {value})"),
                        ("*=", _) => format!("({current} * {value})"),
                        ("/=", _) => format!("KT.div({current}, {value})"),
                        _ => format!("({current} % {value})"),
                    }
                };
                let code = match target {
                    // `+=` on a `val` (a mutable list, a builder) adds to it in place.
                    Expr::Name(name)
                        if self.is_local(name)
                            && matches!(*op, "+=" | "-=")
                            && self.resolve(name).is_some_and(|js| self.vals.contains(&js)) =>
                    {
                        let name = self.resolve(name).unwrap_or_else(|| local(name));
                        format!("{};", combined(name.clone()))
                    }
                    Expr::Name(name) if self.is_local(name) => {
                        let name = self.resolve(name).unwrap_or_else(|| local(name));
                        format!("{name} = {};", combined(name.clone()))
                    }
                    Expr::Name(name) => {
                        let receivers = self.receiver_list();
                        let current = format!("(await KT.rget({receivers}, {}))", quote(name));
                        format!(
                            "await KT.rset({receivers}, {}, {});",
                            quote(name),
                            combined(current)
                        )
                    }
                    Expr::Member { target, name, .. } => {
                        let object = self.expr(target)?;
                        let temp = self.temp();
                        let current = format!("(await KT.p({temp}, {}))", quote(name));
                        format!(
                            "{{ const {temp} = {object}; await KT.setp({temp}, {}, {}); }}",
                            quote(name),
                            combined(current)
                        )
                    }
                    Expr::Index { target, index } if index.len() == 1 => {
                        let object = self.expr(target)?;
                        let key = self.expr(&index[0])?;
                        let (temp, key_temp) = (self.temp(), self.temp());
                        let current = format!("(await KT.m({temp}, \"get\", [{key_temp}]))");
                        format!(
                            "{{ const {temp} = {object}; const {key_temp} = {key}; await KT.m({temp}, \"set\", [{key_temp}, {}]); }}",
                            combined(current)
                        )
                    }
                    _ => return Err(unsupported("an assignment to that")),
                };
                if returned {
                    format!("{code} return KT.unit;")
                } else {
                    code
                }
            }
            Stmt::Expr(expr) => self.expression_statement(expr, returned)?,
            Stmt::For {
                label,
                vars,
                iterable,
                body,
            } => {
                let kind = self.range_num(iterable);
                // A sequence a builder makes (`for (p in countUp(1))`) is read as the loop goes,
                // as it may not end; anything else is gone through as a list.
                let lazy = self.gives_a_sequence(iterable);
                let iterable = self.expr(iterable)?;
                let (each, items) = if lazy {
                    ("for await", format!("KT.items({iterable})"))
                } else {
                    ("for", format!("(await KT.each({iterable}))"))
                };
                let id = self.enter_loop(label.as_ref());
                let code = self.scoped(|writer| -> Result<String, String> {
                    let head = match vars {
                        LambdaParam::Name(name) => {
                            let js = writer.declare_number(name, kind);
                            format!("const {js} of {items}")
                        }
                        LambdaParam::Destructured(names) => {
                            let temp = writer.temp();
                            let mut parts = Vec::new();
                            for (index, name) in names.iter().enumerate() {
                                if name != "_" {
                                    let js = writer.declare(name);
                                    parts.push(format!(
                                        "const {js} = KT.component({temp}, {index});"
                                    ));
                                }
                            }
                            let inner = writer.block(body, Tail::Discard)?;
                            let inner = writer.loop_body(id, inner);
                            return Ok(format!(
                                "{}{each} (const {temp} of {items}) {{ {} {inner} }}",
                                label
                                    .as_ref()
                                    .map(|label| format!("{}: ", local(label)))
                                    .unwrap_or_default(),
                                parts.join(" "),
                            ));
                        }
                    };
                    let inner = writer.block(body, Tail::Discard)?;
                    let inner = writer.loop_body(id, inner);
                    Ok(format!(
                        "{}{each} ({head}) {{ {inner} }}",
                        label
                            .as_ref()
                            .map(|label| format!("{}: ", local(label)))
                            .unwrap_or_default(),
                    ))
                });
                self.loops.pop();
                let code = code?;
                if returned {
                    format!("{code} return KT.unit;")
                } else {
                    code
                }
            }
            Stmt::While {
                label,
                condition,
                body,
                after: true,
            } => {
                let code = self.do_while(label.as_ref(), condition, body)?;
                if returned {
                    format!("{code} return KT.unit;")
                } else {
                    code
                }
            }
            Stmt::While {
                label,
                condition,
                body,
                after,
            } => {
                let condition = self.expr(condition)?;
                let id = self.enter_loop(label.as_ref());
                let body = self.block(body, Tail::Discard);
                self.loops.pop();
                let body = body?;
                let body = self.loop_body(id, body);
                let label = label
                    .as_ref()
                    .map(|label| format!("{}: ", local(label)))
                    .unwrap_or_default();
                let code = if *after {
                    format!("{label}do {{ {body} }} while (KT.truthy({condition}));")
                } else {
                    format!("{label}while (KT.truthy({condition})) {{ {body} }}")
                };
                if returned {
                    format!("{code} return KT.unit;")
                } else {
                    code
                }
            }
            Stmt::Decl(member) => match member.as_ref() {
                Member::Fun(fun) => {
                    let js = self.declare(&fun.name);
                    if fun.receiver.is_some() {
                        self.local_extensions.insert(js.clone());
                    }
                    let function = self.function(fun)?;
                    format!("const {js} = {function};")
                }
                _ => return Err(unsupported("a local class")),
            },
        })
    }

    /// An expression as a statement: control flow stays statements, the rest is evaluated.
    fn expression_statement(&mut self, expr: &Expr, returned: bool) -> Result<String, String> {
        match expr {
            Expr::Return { label, value } => {
                self.return_statement(label.as_deref(), value.as_deref())
            }
            Expr::Throw(value) => Ok(format!("throw KT.error({});", self.expr(value)?)),
            // A loop's own `break`, or one leaving a lambda (`run { break }`) for the loop around
            // it, which the loop's body catches.
            Expr::Break(label) => self.jump_statement("break", label.as_ref()),
            Expr::Continue(label) => self.jump_statement("continue", label.as_ref()),
            Expr::If {
                condition,
                then,
                otherwise,
            } if !returned => {
                let condition = self.expr(condition)?;
                let then = self.block(then, Tail::Discard)?;
                Ok(match otherwise {
                    Some(otherwise) => format!(
                        "if (KT.truthy({condition})) {{ {then} }} else {{ {} }}",
                        self.block(otherwise, Tail::Discard)?
                    ),
                    None => format!("if (KT.truthy({condition})) {{ {then} }}"),
                })
            }
            Expr::When {
                subject,
                binding,
                branches,
            } if !returned => self.when_statement(
                subject.as_deref(),
                binding.as_deref(),
                branches,
                Tail::Discard,
            ),
            Expr::Try {
                body,
                catches,
                finally,
            } if !returned => self.try_statement(body, catches, finally.as_ref(), Tail::Discard),
            _ => {
                let value = self.expr(expr)?;
                Ok(if returned {
                    format!("return {value};")
                } else {
                    format!("{value};")
                })
            }
        }
    }

    fn return_statement(
        &mut self,
        label: Option<&str>,
        value: Option<&Expr>,
    ) -> Result<String, String> {
        // `return client.get(url).parseAs()`: decoded as the function's return type.
        if label.is_none() && self.depth == 0 && value.is_some_and(untyped_decoder) {
            self.expected = self.return_type.clone();
        }
        let value = match value {
            Some(value) => self.expr(value)?,
            None => "KT.unit".to_owned(),
        };
        match label {
            // From the lambda being made: its own `return`.
            Some(label) if self.labels.last().map(String::as_str) == Some(label) => {
                Ok(format!("return {value};"))
            }
            // From a lambda around this one: it catches the return by its label.
            Some(label) if self.labels.iter().any(|known| known == label) => {
                Ok(format!("throw new KT.Return({value}, {});", quote(label)))
            }
            // `return@fetchChapters`: the function's own return.
            Some(_) | None if self.depth == 0 => Ok(format!("return {value};")),
            Some(_) => {
                self.catches_return = true;
                Ok(format!("throw new KT.Return({value});"))
            }
            None => {
                self.catches_return = true;
                Ok(format!("throw new KT.Return({value});"))
            }
        }
    }

    fn when_statement(
        &mut self,
        subject: Option<&Expr>,
        binding: Option<&str>,
        branches: &[WhenBranch],
        tail: Tail,
    ) -> Result<String, String> {
        let subject_name = self.temp();
        let mut code = String::new();
        if let Some(subject) = subject {
            code.push_str(&format!("const {subject_name} = {};", self.expr(subject)?));
        }
        self.scoped(|writer| -> Result<(), String> {
            if let Some(binding) = binding {
                let js = writer.declare(binding);
                code.push_str(&format!(" const {js} = {subject_name};"));
            }
            let mut first = true;
            let mut has_else = false;
            for branch in branches {
                let test = if branch.conditions.is_empty() {
                    has_else = true;
                    None
                } else {
                    let mut tests = Vec::new();
                    for condition in &branch.conditions {
                        tests.push(
                            writer.condition(condition, subject.map(|_| subject_name.as_str()))?,
                        );
                    }
                    let mut test = tests.join(" || ");
                    if let Some(guard) = &branch.guard {
                        test = format!("({test}) && KT.truthy({})", writer.expr(guard)?);
                    }
                    Some(test)
                };
                let body = writer.block(&branch.body, tail)?;
                match test {
                    Some(test) => {
                        code.push_str(&format!(
                            "{}if ({test}) {{ {body} }}",
                            if first { " " } else { " else " }
                        ));
                    }
                    None if first => code.push_str(&format!(" {{ {body} }}")),
                    None => code.push_str(&format!(" else {{ {body} }}")),
                }
                first = false;
            }
            if tail == Tail::Return && !has_else {
                code.push_str(" return KT.unit;");
            }
            Ok(())
        })?;
        Ok(format!("{{ {code} }}"))
    }

    fn condition(
        &mut self,
        condition: &WhenCondition,
        subject: Option<&str>,
    ) -> Result<String, String> {
        Ok(match (condition, subject) {
            (WhenCondition::Expr(value), Some(subject)) => {
                format!("KT.eq({subject}, {})", self.expr(value)?)
            }
            (WhenCondition::Expr(value), None) => format!("KT.truthy({})", self.expr(value)?),
            (WhenCondition::Is { ty, negated }, Some(subject)) => {
                let test = format!("KT.is({subject}, {})", quote(&ty.name));
                if *negated { format!("!{test}") } else { test }
            }
            (WhenCondition::In { range, negated }, Some(subject)) => {
                let test = format!("KT.contains({}, {subject})", self.expr(range)?);
                if *negated { format!("!{test}") } else { test }
            }
            _ => return Err(unsupported("that when")),
        })
    }

    fn try_statement(
        &mut self,
        body: &Block,
        catches: &[Catch],
        finally: Option<&Block>,
        tail: Tail,
    ) -> Result<String, String> {
        let body = self.block(body, tail)?;
        let mut code = format!("try {{ {body} }}");
        if !catches.is_empty() {
            let error = self.temp();
            let mut handlers = String::new();
            for (index, catch) in catches.iter().enumerate() {
                let handler = self.scoped(|writer| -> Result<String, String> {
                    let js = writer.declare(&catch.name);
                    Ok(format!(
                        "const {js} = {error}; {}",
                        writer.block(&catch.body, tail)?
                    ))
                })?;
                // A Kotlin return passing through isn't a failure to catch.
                let test = format!(
                    "!(({error}) instanceof KT.Return) && KT.caught({error}, {})",
                    quote(&catch.ty.name)
                );
                handlers.push_str(&format!(
                    "{}if ({test}) {{ {handler} }}",
                    if index == 0 { "" } else { " else " }
                ));
            }
            code.push_str(&format!(
                " catch ({error}) {{ {handlers} else {{ throw {error}; }} }}"
            ));
        }
        if let Some(finally) = finally {
            code.push_str(&format!(
                " finally {{ {} }}",
                self.block(finally, Tail::Discard)?
            ));
        }
        Ok(code)
    }

    // ---- Expressions ----

    /// An async function of no arguments around a block, called at once: a block as a value.
    fn iife(
        &mut self,
        make: impl FnOnce(&mut Self) -> Result<String, String>,
    ) -> Result<String, String> {
        self.depth += 1;
        let body = make(self);
        self.depth -= 1;
        Ok(format!("(await (async () => {{ {} }})())", body?))
    }

    pub fn expr(&mut self, expr: &Expr) -> Result<String, String> {
        if matches!(expr, Expr::Member { .. })
            && let Some(stripped) = self.without_package(expr)
        {
            return self.expr(&stripped);
        }
        Ok(match expr {
            Expr::Int(value) | Expr::Long(value) => value.to_string(),
            Expr::Float(value) => {
                if value.is_finite() {
                    format!("{value:?}")
                } else {
                    return Err(unsupported("that number"));
                }
            }
            Expr::Bool(value) => value.to_string(),
            Expr::Null => "null".to_owned(),
            Expr::Char(value) => quote(&value.to_string()),
            Expr::Str(parts) => {
                let mut pieces = Vec::new();
                for part in parts {
                    match part {
                        StrPart::Text(text) => pieces.push(quote(text)),
                        StrPart::Code(code) => pieces.push(format!("KT.str({})", self.expr(code)?)),
                    }
                }
                match pieces.len() {
                    0 => "\"\"".to_owned(),
                    1 => pieces.remove(0),
                    _ => format!("({})", pieces.join(" + ")),
                }
            }
            Expr::Name(name) => self.name(name)?,
            Expr::This(label) => match (label, self.receivers.last()) {
                (None, Some(receiver)) => receiver.clone(),
                (Some(label), _) => self.labelled_this(label),
                (None, None) => "self".to_owned(),
            },
            Expr::Super => "self.base".to_owned(),
            Expr::Member { target, name, safe } => self.member(target, name, *safe)?,
            Expr::Call {
                callee,
                type_args,
                args,
                lambda,
            } => self.call(callee, type_args, args, lambda.as_deref())?,
            Expr::Index { target, index } => {
                let object = self.expr(target)?;
                let keys = index
                    .iter()
                    .map(|key| self.expr(key))
                    .collect::<Result<Vec<_>, _>>()?;
                format!("(await KT.m({object}, \"get\", [{}]))", keys.join(", "))
            }
            Expr::NotNull(inner) => format!("KT.nn({})", self.expr(inner)?),
            Expr::Unary { op, expr } if matches!(*op, "++" | "--") => {
                self.increment(op, expr, false)?
            }
            Expr::Unary { op, expr } => {
                let value = self.expr(expr)?;
                match *op {
                    "!" => format!("(!KT.truthy({value}))"),
                    "-" => format!("(-{value})"),
                    _ => value,
                }
            }
            // `i++` as a value: the value before, then the variable moves on.
            Expr::Postfix { op, expr } => self.increment(op, expr, true)?,
            Expr::Binary { op, left, right } => self.binary(op, left, right)?,
            Expr::Is { expr, ty, negated } => {
                let test = format!("KT.is({}, {})", self.expr(expr)?, quote(&ty.name));
                if *negated { format!("(!{test})") } else { test }
            }
            Expr::As { expr, ty, safe } => {
                let value = self.expr(expr)?;
                if *safe {
                    format!("KT.asOrNull({value}, {})", quote(&ty.name))
                } else {
                    value
                }
            }
            Expr::If {
                condition,
                then,
                otherwise,
            } => {
                let simple = |block: &Block| matches!(block.as_slice(), [Stmt::Expr(expr)] if is_plain(expr));
                match otherwise {
                    Some(otherwise) if simple(then) && simple(otherwise) => {
                        let (Stmt::Expr(then), Stmt::Expr(otherwise)) = (&then[0], &otherwise[0])
                        else {
                            unreachable!()
                        };
                        format!(
                            "(KT.truthy({}) ? {} : {})",
                            self.expr(condition)?,
                            self.expr(then)?,
                            self.expr(otherwise)?
                        )
                    }
                    _ => {
                        let condition = self.expr(condition)?;
                        self.iife(|writer| {
                            let then = writer.block(then, Tail::Return)?;
                            let otherwise = match otherwise {
                                Some(otherwise) => writer.block(otherwise, Tail::Return)?,
                                None => "return KT.unit;".to_owned(),
                            };
                            Ok(format!(
                                "if (KT.truthy({condition})) {{ {then} }} else {{ {otherwise} }}"
                            ))
                        })?
                    }
                }
            }
            Expr::When {
                subject,
                binding,
                branches,
            } => {
                let (subject, binding) = (subject.clone(), binding.clone());
                self.iife(|writer| {
                    writer.when_statement(
                        subject.as_deref(),
                        binding.as_deref(),
                        branches,
                        Tail::Return,
                    )
                })?
            }
            Expr::Try {
                body,
                catches,
                finally,
            } => self.iife(|writer| {
                writer.try_statement(body, catches, finally.as_ref(), Tail::Return)
            })?,
            Expr::Lambda(lambda) => self.lambda(lambda, "")?,
            Expr::Reference { target, name } => self.reference(target.as_deref(), name)?,
            Expr::Throw(value) => format!("KT.raise({})", self.expr(value)?),
            Expr::Return { label, value } => {
                let value = match value {
                    Some(value) => self.expr(value)?,
                    None => "KT.unit".to_owned(),
                };
                match label {
                    // A labelled return in an expression position: the lambda with the label
                    // catches it, this one or one around it.
                    Some(label) if self.labels.iter().any(|known| known == label) => {
                        format!("KT.ret({value}, {})", quote(label))
                    }
                    // `return@fetchChapters`: the function's own return.
                    Some(_) | None => {
                        self.catches_return = true;
                        format!("KT.ret({value})")
                    }
                }
            }
            // `?: continue`, `if (…) x else break`: a jump the loop's body catches.
            Expr::Break(label) => self.jump_expression("break", label.as_ref())?,
            Expr::Continue(label) => self.jump_expression("continue", label.as_ref())?,
            Expr::Object(object) => self.anonymous_object(object)?,
        })
    }

    /// `object : Callback { override fun onResponse(…) }`: an object of its functions and values,
    /// which see the code around them; `this` in them is the object.
    fn anonymous_object(&mut self, object: &ClassDecl) -> Result<String, String> {
        let temp = self.temp();
        let supers: Vec<String> = object
            .supers
            .iter()
            .map(|parent| quote(parent.ty.name.rsplit('.').next().unwrap_or(&parent.ty.name)))
            .collect();
        let mut parts = vec![
            "__class: \"object\"".to_owned(),
            format!("__classes: [\"object\", {}]", supers.join(", ")),
        ];
        let mut values = Vec::new();
        for member in &object.members {
            match member {
                Member::Fun(fun) if fun.receiver.is_none() && fun.body.is_some() => {
                    let mut method = fun.clone();
                    // The object is the method's receiver, passed in by the wrapper below.
                    method.receiver = Some(Type {
                        name: "object".into(),
                        ..Default::default()
                    });
                    let function = self.function(&method)?;
                    parts.push(format!(
                        "{}: (...args) => ({function})({temp}, ...args)",
                        quote(&fun.name)
                    ));
                }
                Member::Prop(prop) if prop.receiver.is_none() && prop.getter.is_none() => {
                    let value = match &prop.value {
                        Some(value) => self.expr(value)?,
                        None => "null".to_owned(),
                    };
                    values.push(format!("{temp}[{}] = {value};", quote(&prop.name)));
                }
                _ => return Err(unsupported("that anonymous object")),
            }
        }
        Ok(format!(
            "(await (async () => {{ const {temp} = {{ {} }}; {} return {temp}; }})())",
            parts.join(", "),
            values.join(" ")
        ))
    }

    fn name(&mut self, name: &str) -> Result<String, String> {
        if let Some(js) = self.resolve(name) {
            return Ok(js);
        }
        if name == "it" {
            return Err(unsupported("`it` outside a lambda"));
        }
        if let Some(value) = self.companion_value(name) {
            return Ok(value);
        }
        // `entries` in an enum's own code (its companion's `entries.firstOrNull { … }`).
        if name == "entries"
            && let Some(scope) = self.class_scope.as_deref()
            && self.cx.enum_entries.contains_key(scope)
        {
            return Ok(format!("(await KT.p(D.{scope}, \"entries\"))"));
        }
        // A constant of the runtime's imported by name (`import kotlin.text.RegexOption.IGNORE_CASE`).
        if name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            && let Some(value) = self.imported_runtime_member(name)
        {
            return Ok(value);
        }
        // Implicit receivers first, as Kotlin resolves them, then the source.
        if !self.receivers.is_empty() {
            return Ok(format!(
                "(await KT.rget({}, {}))",
                self.receiver_list(),
                quote(name)
            ));
        }
        if self.cx.properties.contains(name) {
            return Ok(format!("(await self.get({}))", quote(name)));
        }
        if self.cx.constants.contains(name) {
            return Ok(format!("(await C.get({}))", quote(name)));
        }
        if self.cx.classes.contains(name) {
            return Ok(format!("D.{name}"));
        }
        if self.cx.functions.contains_key(name) {
            // A function read as a value: a reference to it.
            return Ok(format!(
                "(async (...args) => self.call({}, args))",
                quote(name)
            ));
        }
        // Anything else is the runtime's, or nothing it could find when the code runs.
        if OBJECTS.contains(&name) || GLOBALS.contains(&name) {
            return Ok(format!("KT.global({})", quote(name)));
        }
        // An enum's entry by itself: in the enum's own code, or imported
        // (`import ….Filter.TriState.Companion.STATE_IGNORE`, `import ….Method.SINGLE`).
        if let Some(entry) = self.enum_entry(name) {
            return Ok(entry);
        }
        // `import java.time.Instant as JavaInstant`: the runtime's object under another name.
        if let Some(real) = self.aliased(name)
            && (OBJECTS.contains(&real.as_str()) || GLOBALS.contains(&real.as_str()))
        {
            return Ok(format!("KT.global({})", quote(&real)));
        }
        Err(unsupported(name))
    }

    /// An object the runtime has, as the owner of a member (`Calendar.getInstance()`), under its
    /// own name or one an import gives it (`JavaInstant.ofEpochMilli`).
    fn runtime_object(&self, owner: &str, name: &str) -> Result<String, String> {
        if OBJECTS.contains(&owner) {
            Ok(format!("KT.global({})", quote(owner)))
        } else if let Some(real) = self
            .aliased(owner)
            .filter(|real| OBJECTS.contains(&real.as_str()))
        {
            Ok(format!("KT.global({})", quote(&real)))
        } else {
            Err(unsupported(format!("{owner}.{name}")))
        }
    }

    /// The name an import alias stands for (`import java.time.Instant as JavaInstant`).
    fn aliased(&self, alias: &str) -> Option<String> {
        self.imports
            .iter()
            .find(|(name, full)| name == alias && !full.ends_with(&format!(".{alias}")))
            .and_then(|(_, full)| full.rsplit('.').next())
            .map(str::to_owned)
    }

    /// A member of one of the runtime's objects imported by name (`import
    /// kotlin.text.RegexOption.IGNORE_CASE`, `import java.util.concurrent.TimeUnit.SECONDS`).
    fn imported_runtime_member(&self, name: &str) -> Option<String> {
        let (_, full) = self.imports.iter().find(|(alias, _)| alias == name)?;
        let mut parts = full.rsplit('.');
        let member = parts.next()?;
        let owner = parts.find(|part| *part != "Companion")?;
        (OBJECTS.contains(&owner) && !self.cx.classes.contains(owner)).then(|| {
            format!(
                "(await KT.p(KT.global({}), {}))",
                quote(owner),
                quote(member)
            )
        })
    }

    /// An enum entry named by itself: of the enum whose code this is, or one imported by name.
    fn enum_entry(&self, name: &str) -> Option<String> {
        if let Some(scope) = self.class_scope.as_deref()
            && self
                .cx
                .enum_entries
                .get(scope)
                .is_some_and(|entries| entries.contains(name))
        {
            return Some(format!("(await KT.p(D.{scope}, {}))", quote(name)));
        }
        let (_, full) = self.imports.iter().find(|(alias, _)| alias == name)?;
        let mut parts = full.rsplit('.');
        let entry = parts.next()?;
        let owner = parts.find(|part| *part != "Companion")?;
        if self
            .cx
            .enum_entries
            .get(owner)
            .is_some_and(|entries| entries.contains(entry))
        {
            return Some(format!("(await KT.p(D.{owner}, {}))", quote(entry)));
        }
        None
    }

    fn member(&mut self, target: &Expr, name: &str, safe: bool) -> Result<String, String> {
        if let Expr::Super = target {
            return Ok(format!("(await self.base.get({}))", quote(name)));
        }
        // `ReadRealm.IMAGE_URL` in a DTO: a constant of the source's own companion.
        if let Expr::Name(owner) = target
            && self.cx.source_classes.contains(owner)
            && !self.is_local(owner)
            && self.cx.constants.contains(name)
        {
            return Ok(format!("(await C.get({}))", quote(name)));
        }
        // `Page::class.java`: the class (its name, and the extension's resources).
        if matches!(name, "java" | "javaObjectType" | "kotlin")
            && matches!(target, Expr::Reference { name, .. } if name == "class")
        {
            let class = self.expr(target)?;
            return Ok(format!("KT.javaClass({class})"));
        }
        // `this.baseUrl` in the source: its own property.
        if matches!(target, Expr::This(None))
            && self.receivers.is_empty()
            && self.class_scope.is_none()
            && self.cx.properties.contains(name)
        {
            return Ok(format!("(await self.get({}))", quote(name)));
        }
        // A companion's or an object's member (`Companion.X`, `SManga.ONGOING`).
        if let Expr::Name(owner) = target
            && !self.is_local(owner)
            && owner.chars().next().is_some_and(char::is_uppercase)
            && self.receivers.is_empty()
            && !self.cx.properties.contains(owner)
            && !self.cx.constants.contains(owner)
            && self.companion_value(owner).is_none()
        {
            if self
                .cx
                .companions
                .get(owner)
                .is_some_and(|names| names.contains(name))
            {
                return Ok(format!(
                    "(await C.get({}))",
                    quote(&companion_key(owner, name))
                ));
            }
            if self.cx.classes.contains(owner) {
                return Ok(format!("(await KT.p(D.{owner}, {}))", quote(name)));
            }
            let object = self.runtime_object(owner, name)?;
            return Ok(format!("(await KT.p({object}, {}))", quote(name)));
        }
        let object = self.expr(target)?;
        if self.cx.extension_properties.contains(name) {
            let call = format!("(await self.ext({}, {object}, []))", quote(name));
            return Ok(if safe {
                format!(
                    "(await KT.safe({object}, async (v) => self.ext({}, v, [])))",
                    quote(name)
                )
            } else {
                call
            });
        }
        Ok(if safe {
            format!("(await KT.ps({object}, {}))", quote(name))
        } else {
            format!("(await KT.p({object}, {}))", quote(name))
        })
    }

    /// Arguments in order, named ones placed by `params` (a function of the source's); for others,
    /// named arguments go in an object after them.
    fn args(
        &mut self,
        args: &[Arg],
        params: Option<&[String]>,
    ) -> Result<(Vec<String>, Option<String>), String> {
        let mut positional = Vec::new();
        let mut named = Vec::new();
        for arg in args {
            let value = self.expr(&arg.value)?;
            let value = if arg.spread {
                format!("...KT.iter({value})")
            } else {
                value
            };
            match &arg.name {
                None => positional.push(value),
                Some(name) => named.push((name.clone(), value)),
            }
        }
        // A vararg before other parameters (`listUrl(vararg segments, page)`) takes the
        // positional arguments as one list.
        if let Some(params) = params
            && let Some(at) = params.iter().position(|param| param.starts_with('*'))
        {
            let mut placed: Vec<Option<String>> = vec![None; params.len()];
            placed[at] = Some(format!("[{}]", positional.join(", ")));
            for (name, value) in named {
                let index = params
                    .iter()
                    .position(|param| param.trim_start_matches(['*', '^']) == name)
                    .ok_or_else(|| unsupported(format!("a named argument {name}")))?;
                placed[index] = Some(value);
            }
            return Ok((
                placed
                    .into_iter()
                    .map(|value| value.unwrap_or_else(|| "undefined".to_owned()))
                    .collect(),
                None,
            ));
        }
        if named.is_empty() {
            return Ok((positional, None));
        }
        if let Some(params) = params {
            let mut placed: Vec<Option<String>> = positional.into_iter().map(Some).collect();
            for (name, value) in named {
                let index = params
                    .iter()
                    .position(|param| param.trim_start_matches('^') == name)
                    .ok_or_else(|| unsupported(format!("a named argument {name}")))?;
                if placed.len() <= index {
                    placed.resize(index + 1, None);
                }
                placed[index] = Some(value);
            }
            return Ok((
                placed
                    .into_iter()
                    .map(|value| value.unwrap_or_else(|| "undefined".to_owned()))
                    .collect(),
                None,
            ));
        }
        let object = named
            .iter()
            .map(|(name, value)| format!("{}: {value}", quote(name)))
            .collect::<Vec<_>>()
            .join(", ");
        Ok((positional, Some(format!("{{{object}}}"))))
    }

    /// The overload of one of the source's functions that takes these arguments.
    fn own_params(&self, name: &str, args: &[Arg]) -> Option<Vec<String>> {
        let overloads = self.cx.functions.get(name)?;
        let named: Vec<&str> = args.iter().filter_map(|arg| arg.name.as_deref()).collect();
        overloads
            .iter()
            .find(|params| {
                let vararg = params.iter().any(|param| param.starts_with('*'));
                (vararg || params.len() >= args.len())
                    && named.iter().all(|name| {
                        params
                            .iter()
                            .any(|param| param.trim_start_matches(['*', '^']) == *name)
                    })
            })
            .or_else(|| overloads.first())
            .cloned()
    }

    fn call(
        &mut self,
        callee: &Expr,
        type_args: &[Type],
        args: &[Arg],
        lambda: Option<&Lambda>,
    ) -> Result<String, String> {
        if let Some(stripped) = self.without_package(callee) {
            return self.call(&stripped, type_args, args, lambda);
        }
        let expected = self.expected.take();
        let type_arg = type_args
            .first()
            .or(expected.as_ref())
            .map(|ty| self.type_code(&type_name(&self.cx.unaliased(ty))))
            .unwrap_or_else(|| "null".to_owned());
        // A call to one of the source's functions with reified type parameters passes its
        // types along (`parseAsNextData<MangaListDto>()`).
        let passed_types: Option<String> = match callee {
            Expr::Name(name) | Expr::Member { name, .. } => {
                self.cx.reified.get(name).map(|count| {
                    let mut types: Vec<String> = type_args
                        .iter()
                        .map(|ty| self.type_code(&type_name(&self.cx.unaliased(ty))))
                        .collect();
                    if types.is_empty()
                        && let Some(ty) = expected.as_ref()
                    {
                        types.push(self.type_code(&type_name(&self.cx.unaliased(ty))));
                    }
                    types.truncate(*count);
                    format!("[{}]", types.join(", "))
                })
            }
            _ => None,
        };
        match callee {
            Expr::Name(name) => {
                // Scope functions without a receiver.
                match (name.as_str(), lambda) {
                    // `apply { … }` alone, in an extension or a scope: on `this`.
                    ("apply" | "also" | "let" | "takeIf" | "takeUnless", Some(_))
                        if args.is_empty() && !self.receivers.is_empty() =>
                    {
                        let on_this = Expr::Member {
                            target: Box::new(Expr::This(None)),
                            name: name.clone(),
                            safe: false,
                        };
                        return self.call(&on_this, type_args, args, lambda);
                    }
                    ("run", Some(lambda)) if args.is_empty() => {
                        return self.lambda_call(lambda, "run");
                    }
                    ("with", Some(lambda)) if args.len() == 1 => {
                        let value = self.expr(&args[0].value)?;
                        return self.receiver_call(lambda, &value, "with");
                    }
                    ("buildList" | "buildSet" | "mutableListOf", Some(lambda))
                        if name != "mutableListOf" =>
                    {
                        return self.receiver_call_returning(lambda, "[]", name);
                    }
                    // Keiyoushi's `runWebView { … }`: a web view the source drives, which Raygun
                    // doesn't have. The call fails naming the page it would load: as a browser
                    // check when the function passes one (which the person may then pass by
                    // hand), else as a page that needs a web view.
                    ("runWebView" | "runWebViewBlocking", Some(lambda)) => {
                        let page = loaded_page(&lambda.body)
                            .and_then(|page| self.expr(&page).ok())
                            .unwrap_or_else(|| "null".to_owned());
                        let check = passes_a_check(&self.function_name);
                        return Ok(format!(
                            "(await KT.call(\"runWebView\", [{page}, (await self.get(\"baseUrl\")), {check}]))"
                        ));
                    }
                    // kotlinx.serialization's settings: reading always ignores unknown keys and
                    // tells subclasses by their discriminator, so only that is read.
                    ("Json", Some(lambda)) => {
                        let discriminator = lambda.body.iter().find_map(|stmt| match stmt {
                            Stmt::Assign {
                                target: Expr::Name(name),
                                value: Expr::Str(parts),
                                ..
                            } if name == "classDiscriminator" => match parts.as_slice() {
                                [StrPart::Text(text)] => Some(text.clone()),
                                _ => None,
                            },
                            _ => None,
                        });
                        return Ok(match discriminator {
                            Some(key) => format!(
                                "(await KT.call(\"Json\", [], {{ classDiscriminator: {} }}))",
                                quote(&key)
                            ),
                            None => "(await KT.call(\"Json\", []))".to_owned(),
                        });
                    }
                    ("SerializersModule", Some(_)) => return Ok("null".to_owned()),
                    // `lazy(::make)`: the function's value, once.
                    ("lazy", None) if args.len() == 1 => {
                        let function = self.expr(&args[0].value)?;
                        return Ok(format!(
                            "KT.deferred(async () => (await KT.invoke({function}, [])))"
                        ));
                    }
                    ("buildMap", Some(lambda)) => {
                        return self.receiver_call_returning(lambda, "new Map()", name);
                    }
                    // `sequence { yield(x); yieldAll(xs) }`, `iterator { … }`: a sequence whose
                    // block runs as its items are read, with its scope as the receiver.
                    ("sequence" | "iterator", Some(lambda))
                        if args.is_empty() && !self.cx.functions.contains_key(name) =>
                    {
                        let block = self.receiver_lambda(lambda, name)?;
                        let made = format!("KT.sequenceOf({block})");
                        return Ok(if name == "iterator" {
                            format!("(await KT.m({made}, \"iterator\", []))")
                        } else {
                            made
                        });
                    }
                    ("buildString", Some(lambda)) => {
                        let built =
                            self.receiver_call_returning(lambda, "KT.stringBuilder()", name)?;
                        return Ok(format!("KT.str({built})"));
                    }
                    ("buildJsonObject", Some(lambda)) => {
                        return self
                            .receiver_call_returning(lambda, "KT.jsonBuilder()", name)
                            .map(|built| format!("KT.built({built})"));
                    }
                    ("buildJsonArray", Some(lambda)) => {
                        return self
                            .receiver_call_returning(lambda, "KT.jsonArrayBuilder()", name)
                            .map(|built| format!("KT.built({built})"));
                    }
                    ("runCatching", Some(lambda)) => {
                        let call = self.lambda_call(lambda, name)?;
                        return Ok(format!("(await KT.runCatching(async () => {call}))"));
                    }
                    // `getPreferencesLazy { … }`: the preferences, after a block that reads them
                    // as its receiver (moving an old setting to a new one).
                    ("getPreferences" | "getPreferencesLazy", Some(lambda)) if args.is_empty() => {
                        let preferences = self.receiver_call_returning(
                            lambda,
                            "(await KT.call(\"getPreferences\", []))",
                            name,
                        )?;
                        return Ok(if name == "getPreferences" {
                            preferences
                        } else {
                            format!("KT.deferred(async () => {preferences})")
                        });
                    }
                    ("repeat", Some(lambda)) if args.len() == 1 => {
                        let times = self.expr(&args[0].value)?;
                        let function = self.lambda(lambda, name)?;
                        return Ok(format!("(await KT.repeat({times}, {function}))"));
                    }
                    (
                        "coroutineScope" | "supervisorScope" | "withContext" | "runBlocking",
                        Some(lambda),
                    ) => {
                        return self.lambda_call(lambda, name);
                    }
                    ("async" | "lazy", Some(lambda)) => {
                        let call = self.lambda_call(lambda, name)?;
                        return Ok(format!("KT.deferred(async () => {call})"));
                    }
                    _ => {}
                }
                if let Some(js) = self
                    .resolve(name)
                    .filter(|js| !(self.plain.contains(js) && self.cx.functions.contains_key(name)))
                {
                    let (args, _) = self.args(args, None)?;
                    let mut args = args;
                    if let Some(lambda) = lambda {
                        args.push(self.lambda(lambda, name)?);
                    }
                    // A local extension (or a parameter that's a function with a receiver) called
                    // on the implicit receiver.
                    if self.local_extensions.contains(&js)
                        || (self.receiver_params.contains(&js) && !self.receivers.is_empty())
                    {
                        let receiver = self
                            .receivers
                            .last()
                            .cloned()
                            .unwrap_or_else(|| "self".to_owned());
                        args.insert(0, receiver);
                    }
                    return Ok(format!("(await {js}({}))", args.join(", ")));
                }
                // A class the source declares, made inside a scope too (`FeedUrl(…)` in `apply`),
                // or through a `typealias` of one.
                if let Some(class) = (self.declared_class(name)
                    && name.chars().next().is_some_and(char::is_uppercase)
                    && !self.cx.functions.contains_key(name))
                .then(|| name.clone())
                .or_else(|| self.aliased_class(name))
                {
                    let (args, named) = self.args(args, None)?;
                    let mut args = args;
                    if let Some(lambda) = lambda {
                        args.push(self.lambda(lambda, name)?);
                    }
                    return Ok(format!(
                        "(await KT.make(D.{class}, [{}], {}))",
                        args.join(", "),
                        named.unwrap_or_else(|| "null".to_owned())
                    ));
                }
                // An implicit receiver's member (`setUrlWithoutDomain(…)` in `apply`).
                // A lambda for one of the source's functions that takes a receiver is written
                // with it (below).
                let takes_receiver = self.own_params(name, args).is_some_and(|params| {
                    params.last().is_some_and(|param| param.starts_with('^'))
                });
                let lambda_code = lambda
                    .filter(|_| !takes_receiver)
                    .map(|lambda| self.trailing_lambda(lambda, name))
                    .transpose()?;
                if !self.receivers.is_empty() && !self.cx.functions.contains_key(name) {
                    // A value of the source's that's a function, called inside a scope
                    // (`name = chapterTitleFormatter(n, title)` in `apply`).
                    if !self.known(name)
                        && !self.cx.extensions.contains(name)
                        && !GLOBALS.contains(&name.as_str())
                        && !self.is_local(name)
                        && (self.cx.constants.contains(name) || self.cx.properties.contains(name))
                    {
                        let function = self.name(name)?;
                        let (mut args, _) = self.args(args, None)?;
                        if let Some(lambda) = lambda_code {
                            args.push(lambda);
                        }
                        return Ok(format!(
                            "(await KT.invoke({function}, [{}]))",
                            args.join(", ")
                        ));
                    }
                    if !self.known(name) && !self.cx.extensions.contains(name) {
                        return self.global_call(name, args, lambda_code, &type_arg);
                    }
                    let (args, named) = self.args(args, None)?;
                    return Ok(format!(
                        "(await KT.rcall({}, {}, {}, {}, {}, {type_arg}))",
                        self.receiver_list(),
                        quote(name),
                        typed_args(&args, passed_types.as_deref()),
                        named.unwrap_or_else(|| "null".to_owned()),
                        lambda_code.unwrap_or_else(|| "null".to_owned())
                    ));
                }
                if let Some(params) = self.own_params(name, args) {
                    let (mut args, _) = self.args(args, Some(&params))?;
                    // A trailing lambda for a function with a receiver reads it implicitly
                    // (`browse(page) { addQueryParameter(…) }`).
                    let receiver_lambda = lambda
                        .filter(|_| params.last().is_some_and(|param| param.starts_with('^')));
                    // A trailing lambda is the last parameter's, past any defaults left out.
                    if lambda.is_some() {
                        while args.len() + 1 < params.len() {
                            args.push("undefined".to_owned());
                        }
                    }
                    if let Some(lambda) = receiver_lambda {
                        args.push(self.receiver_lambda(lambda, name)?);
                    } else if let Some(lambda) = lambda_code {
                        args.push(lambda);
                    }
                    return Ok(format!(
                        "(await self.call({}, {}))",
                        quote(name),
                        typed_args(&args, passed_types.as_deref())
                    ));
                }
                if let Some(class) = self
                    .declared_class(name)
                    .then(|| name.clone())
                    .or_else(|| self.aliased_class(name))
                {
                    let (args, named) = self.args(args, None)?;
                    return Ok(format!(
                        "(await KT.make(D.{class}, [{}], {}))",
                        args.join(", "),
                        named.unwrap_or_else(|| "null".to_owned())
                    ));
                }
                // A value that's a function (`val searchQuery = { source: String, … -> … }`).
                if !GLOBALS.contains(&name.as_str())
                    && (self.cx.constants.contains(name) || self.cx.properties.contains(name))
                {
                    let function = self.expr(callee)?;
                    let (mut args, _) = self.args(args, None)?;
                    if let Some(lambda) = lambda_code {
                        args.push(lambda);
                    }
                    return Ok(format!(
                        "(await KT.invoke({function}, [{}]))",
                        args.join(", ")
                    ));
                }
                self.global_call(name, args, lambda_code, &type_arg)
            }
            Expr::Member { target, name, safe } => {
                // `x?.split(";").orEmpty()`, `dto.titles.orEmpty()`: an empty list, not text.
                let type_arg =
                    if name == "orEmpty" && type_arg == "null" && self.gives_a_list(target) {
                        quote("List")
                    } else {
                        type_arg
                    };
                // Scope functions with a receiver.
                if let Some(lambda) = lambda
                    && args.is_empty()
                    && matches!(
                        name.as_str(),
                        "apply"
                            | "also"
                            | "let"
                            | "run"
                            | "takeIf"
                            | "takeUnless"
                            | "use"
                            | "runCatching"
                    )
                {
                    let value = self.expr(target)?;
                    let call = |writer: &mut Self, value: &str| -> Result<String, String> {
                        match name.as_str() {
                            "apply" => writer.receiver_call_returning(lambda, value, name),
                            "run" => writer.receiver_call(lambda, value, name),
                            // `value.runCatching { … }`: `run`, its failure kept as a result.
                            "runCatching" => {
                                let call = writer.receiver_call(lambda, value, name)?;
                                Ok(format!("(await KT.runCatching(async () => {call}))"))
                            }
                            "also" => {
                                let call = writer.it_call(lambda, value, name)?;
                                Ok(format!("(await KT.also({value}, async () => {call}))"))
                            }
                            // `stream.use { … }`: the block, then the value closed (a
                            // `GZIPOutputStream` writes what it compressed then).
                            "use" => {
                                let call = writer.it_call(lambda, value, name)?;
                                Ok(format!("(await KT.use({value}, async () => {call}))"))
                            }
                            "takeIf" => {
                                let call = writer.it_call(lambda, value, name)?;
                                Ok(format!("(KT.truthy({call}) ? {value} : null)"))
                            }
                            "takeUnless" => {
                                let call = writer.it_call(lambda, value, name)?;
                                Ok(format!("(KT.truthy({call}) ? null : {value})"))
                            }
                            _ => writer.it_call(lambda, value, name),
                        }
                    };
                    let temp = self.temp();
                    let inner = call(self, &temp)?;
                    return Ok(if *safe {
                        format!("(await KT.safe({value}, async ({temp}) => {inner}))")
                    } else {
                        format!("(await (async ({temp}) => {inner})({value}))")
                    });
                }
                if let Expr::Super = target.as_ref() {
                    let (args, _) = self.args(args, None)?;
                    return Ok(format!(
                        "(await self.base.call({}, [{}]))",
                        quote(name),
                        args.join(", ")
                    ));
                }
                // `this.pageListParseAlternative(document)` in the source: its own function.
                if matches!(target.as_ref(), Expr::This(None))
                    && self.receivers.is_empty()
                    && self.class_scope.is_none()
                    && let Some(params) = self.own_params(name, args)
                {
                    let (mut args, _) = self.args(args, Some(&params))?;
                    if let Some(lambda) = lambda {
                        args.push(self.lambda(lambda, name)?);
                    }
                    return Ok(format!(
                        "(await self.call({}, {}))",
                        quote(name),
                        typed_args(&args, passed_types.as_deref())
                    ));
                }
                // `MangaResponse.Props.Chapter(…)`: a class the source declares, written inside the
                // ones it's nested in.
                if self.cx.classes.contains(name.as_str())
                    && matches!(target.as_ref(), Expr::Member { .. } | Expr::Name(_))
                    && self.declared_path(target)
                {
                    let (args, named) = self.args(args, None)?;
                    let mut args = args;
                    if let Some(lambda) = lambda {
                        args.push(self.lambda(lambda, name)?);
                    }
                    return Ok(format!(
                        "(await KT.make(D.{name}, [{}], {}))",
                        args.join(", "),
                        named.unwrap_or_else(|| "null".to_owned())
                    ));
                }
                // `Filter.Sort.Selection(…)`: a class of the runtime's, written inside the ones
                // it's nested in.
                if GLOBALS.contains(&name.as_str())
                    && matches!(target.as_ref(), Expr::Member { .. })
                    && self.is_type_path(target)
                {
                    let lambda = lambda.map(|lambda| self.lambda(lambda, name)).transpose()?;
                    return self.global_call(name, args, lambda, &type_arg);
                }
                // `Foo.create()`, `Jsoup.parse(…)`: an object's member (not a constant's, such as
                // a class's companion's `NUMBER_REGEX.findAll(…)`).
                if let Expr::Name(owner) = target.as_ref()
                    && !self.is_local(owner)
                    && owner.chars().next().is_some_and(char::is_uppercase)
                    && !self.cx.properties.contains(owner)
                    && !self.cx.constants.contains(owner)
                    && self.companion_value(owner).is_none()
                    && !(self.class_scope.is_some() && self.cx.class_members.contains(owner))
                {
                    let owner_code = if self.cx.classes.contains(owner) {
                        format!("D.{owner}")
                    } else {
                        self.runtime_object(owner, name)?
                    };
                    let (args, named) = self.args(args, None)?;
                    let lambda = lambda.map(|lambda| self.lambda(lambda, name)).transpose()?;
                    return Ok(format!(
                        "(await KT.m({owner_code}, {}, [{}], {}, {}, {type_arg}))",
                        quote(name),
                        args.join(", "),
                        named.unwrap_or_else(|| "null".to_owned()),
                        lambda.unwrap_or_else(|| "null".to_owned())
                    ));
                }
                let object = self.expr(target)?;
                // `builder.params()`: a parameter that's a function with a receiver.
                if let Some(js) = self
                    .resolve(name)
                    .filter(|js| self.receiver_params.contains(js))
                {
                    let (mut args, _) = self.args(args, None)?;
                    args.insert(0, object);
                    return Ok(format!("(await {js}({}))", args.join(", ")));
                }
                // `builder.addParam(…)`: a local extension, the value passed first.
                if let Some(js) = self
                    .resolve(name)
                    .filter(|js| self.local_extensions.contains(js))
                {
                    let (mut args, _) = self.args(args, None)?;
                    if let Some(lambda) = lambda {
                        args.push(self.lambda(lambda, name)?);
                    }
                    let temp = self.temp();
                    args.insert(0, temp.clone());
                    let call = format!("{js}({})", args.join(", "));
                    return Ok(if *safe {
                        format!("(await KT.safe({object}, async ({temp}) => {call}))")
                    } else {
                        format!("(await (async ({temp}) => {call})({object}))")
                    });
                }
                let own_extension = self.cx.extensions.contains(name);
                if !own_extension && !self.known(name) {
                    return Err(unsupported(format!("{name}()")));
                }
                let (args, named) = self.args(args, None)?;
                let lambda = lambda
                    .map(|lambda| self.trailing_lambda(lambda, name))
                    .transpose()?;
                let mut call_args = args;
                let lambda_arg = lambda.unwrap_or_else(|| "null".to_owned());
                let call = |receiver: &str| {
                    if own_extension {
                        let mut all = call_args.clone();
                        if lambda_arg != "null" {
                            all.push(lambda_arg.clone());
                        }
                        format!(
                            "self.ext({}, {receiver}, {})",
                            quote(name),
                            typed_args(&all, passed_types.as_deref())
                        )
                    } else {
                        format!(
                            "KT.m({receiver}, {}, [{}], {}, {lambda_arg}, {type_arg})",
                            quote(name),
                            call_args.join(", "),
                            named.clone().unwrap_or_else(|| "null".to_owned())
                        )
                    }
                };
                let code = if *safe {
                    let inner = call("v");
                    format!("(await KT.safe({object}, async (v) => {inner}))")
                } else {
                    format!("(await {})", call(&object))
                };
                call_args.clear();
                Ok(code)
            }
            // `{ … }()`, `f(x)(y)`.
            _ => {
                let function = self.expr(callee)?;
                let (args, _) = self.args(args, None)?;
                Ok(format!(
                    "(await KT.invoke({function}, [{}]))",
                    args.join(", ")
                ))
            }
        }
    }

    fn global_call(
        &mut self,
        name: &str,
        args: &[Arg],
        lambda: Option<String>,
        type_arg: &str,
    ) -> Result<String, String> {
        if !GLOBALS.contains(&name) {
            return Err(unsupported(format!("{name}()")));
        }
        // `Intl(…, classLoader = this::class.java.classLoader!!)`: the messages come with the
        // converted code, so its class loader and file names aren't needed.
        let kept: Vec<Arg>;
        let args = if name == "Intl" {
            kept = args
                .iter()
                .enumerate()
                .filter(|(index, arg)| match arg.name.as_deref() {
                    Some(name) => !matches!(name, "classLoader" | "createMessageFileName"),
                    None => *index < 3,
                })
                .map(|(_, arg)| arg.clone())
                .collect();
            &kept[..]
        } else {
            args
        };
        let (args, named) = self.args(args, None)?;
        Ok(format!(
            "(await KT.call({}, [{}], {}, {}, {type_arg}))",
            quote(name),
            args.join(", "),
            named.unwrap_or_else(|| "null".to_owned()),
            lambda.unwrap_or_else(|| "null".to_owned())
        ))
    }

    fn reference(&mut self, target: Option<&Expr>, name: &str) -> Result<String, String> {
        // `this::class`, `this@Source::class`: the class whose code this is.
        if name == "class"
            && let Some(Expr::This(_)) = target
        {
            return Ok(format!(
                "KT.javaClass({})",
                quote(self.class_scope.as_deref().unwrap_or("Source"))
            ));
        }
        // `exception::class.simpleName`: a value's class.
        if name == "class"
            && let Some(value) = target
            && !matches!(value, Expr::Name(owner) if owner.chars().next().is_some_and(char::is_uppercase) && !self.is_local(owner))
        {
            let value = self.expr(value)?;
            return Ok(format!("KT.javaClass(KT.classNameOf({value}))"));
        }
        // `::node`: a local function, already a function.
        if target.is_none()
            && let Some(js) = self.resolve(name)
        {
            return Ok(js);
        }
        match target {
            // `::helper`: one of the source's functions.
            None if self.cx.functions.contains_key(name) => Ok(format!(
                "(async (...args) => self.call({}, args))",
                quote(name)
            )),
            None if self.cx.extensions.contains(name) => Ok(format!(
                "(async (receiver, ...args) => self.ext({}, receiver, args))",
                quote(name)
            )),
            // `::append` in `buildString { … }`: a member of an implicit receiver.
            None if !self.receivers.is_empty() && self.known(name) => Ok(format!(
                "(async (...args) => KT.rcall({}, {}, args, null, null, null))",
                self.receiver_list(),
                quote(name)
            )),
            // `::listOf`, `::Pair`: one of the runtime's functions.
            None if GLOBALS.contains(&name) => Ok(format!(
                "(async (...args) => KT.call({}, args, null, null, null))",
                quote(name)
            )),
            // `::MangaDto`: a class the source declares, made from the arguments.
            None if self.cx.classes.contains(name) => Ok(format!(
                "(async (...args) => KT.make(D.{name}, args, null))"
            )),
            None => Err(unsupported(format!("::{name}"))),
            // `ChapterDto::nextJsFilter`: a function of the class's companion.
            Some(Expr::Name(owner))
                if self
                    .cx
                    .companion_functions
                    .get(owner)
                    .is_some_and(|functions| functions.contains(name)) =>
            {
                Ok(format!(
                    "(async (...args) => KT.m(D.{owner}, {}, args))",
                    quote(name)
                ))
            }
            // `String::isNotBlank`, `Element::text`: a member of whatever is passed (but
            // `PATTERN::find` is bound to the constant `PATTERN`).
            Some(Expr::Name(owner))
                if owner.chars().next().is_some_and(char::is_uppercase)
                    && !self.is_local(owner)
                    && !self.cx.constants.contains(owner)
                    && !self.cx.properties.contains(owner)
                    && self.companion_value(owner).is_none() =>
            {
                if name == "class" {
                    return Ok(format!("KT.javaClass({})", quote(owner)));
                }
                if self.cx.extensions.contains(name) {
                    return Ok(format!(
                        "(async (receiver, ...args) => self.ext({}, receiver, args))",
                        quote(name)
                    ));
                }
                // `SChapter::chapter_number`: a field of Mihon's models.
                if matches!(owner.as_str(), "SChapter" | "SManga" | "Page") {
                    return Ok(format!("(async (value) => KT.p(value, {}))", quote(name)));
                }
                // `Genre::state`, `TagCheckBox::state`: a declared class's own field or function
                // (or an object's: `Unpacker::unpack`), else a filter's, read on what's passed.
                if self.cx.classes.contains(owner) && self.cx.class_members.contains(name) {
                    return Ok(format!(
                        "(async (...args) => KT.ref(D.{owner}, {}, args))",
                        quote(name)
                    ));
                }
                if matches!(name, "state" | "name" | "values") {
                    return Ok(format!(
                        "(async (receiver, ...args) => KT.member(receiver, {}, args))",
                        quote(name)
                    ));
                }
                if !self.known(name) {
                    return Err(unsupported(format!("{owner}::{name}")));
                }
                // `Instant::parseOrNull`, `BitmapFactory::decodeStream`: one of the runtime's
                // objects' functions when it has one by that name, else a member of what's passed.
                if OBJECTS.contains(&owner.as_str()) && !self.declared_class(owner) {
                    return Ok(format!(
                        "(async (...args) => KT.ref(KT.global({}), {}, args))",
                        quote(owner),
                        quote(name)
                    ));
                }
                // `Unpacker::unpack`: an object the code declares, when it has that function.
                if self.declared_class(owner) {
                    return Ok(format!(
                        "(async (...args) => KT.ref(D.{owner}, {}, args))",
                        quote(name)
                    ));
                }
                Ok(format!(
                    "(async (receiver, ...args) => KT.member(receiver, {}, args))",
                    quote(name)
                ))
            }
            // `list::add`: bound to a value.
            Some(target) => {
                let object = self.expr(target)?;
                if !self.known(name) {
                    return Err(unsupported(format!("::{name}")));
                }
                let temp = self.temp();
                Ok(format!(
                    "(({temp}) => async (...args) => KT.member({temp}, {}, args))({object})",
                    quote(name)
                ))
            }
        }
    }

    fn binary(&mut self, op: &str, left: &Expr, right: &Expr) -> Result<String, String> {
        // `x ?: return`, `x ?: throw …`, `x ?: continue`.
        if op == "?:" {
            let left = self.expr(left)?;
            let right = self.expr(right)?;
            return Ok(format!("({left} ?? {right})"));
        }
        // Whole numbers of a kind the code fixes wrap as Kotlin's do.
        let kind = self.num_binary(op, left, right);
        if kind == Some(Num::Long) && self.has_long_growth(op, left, right) {
            return Ok(format!("KT.fromBig({})", self.exact_long(op, left, right)?));
        }
        let ints = kind == Some(Num::Int) && self.num(left) == Some(Num::Int);
        let (left, right) = (self.expr(left)?, self.expr(right)?);
        if ints {
            match op {
                "+" => return Ok(format!("(({left} + {right}) | 0)")),
                "-" => return Ok(format!("(({left} - {right}) | 0)")),
                "*" => return Ok(format!("Math.imul({left}, {right})")),
                "and" => return Ok(format!("({left} & {right})")),
                "or" => return Ok(format!("({left} | {right})")),
                "xor" => return Ok(format!("({left} ^ {right})")),
                "shl" => return Ok(format!("({left} << {right})")),
                "shr" => return Ok(format!("({left} >> {right})")),
                "ushr" => return Ok(format!("(({left} >>> {right}) | 0)")),
                _ => {}
            }
        }
        Ok(match op {
            "+" => format!("KT.plus({left}, {right})"),
            "-" => format!("KT.minus({left}, {right})"),
            "*" => format!("({left} * {right})"),
            "/" => format!("KT.div({left}, {right})"),
            "%" => format!("({left} % {right})"),
            "==" => format!("KT.eq({left}, {right})"),
            "!=" => format!("(!KT.eq({left}, {right}))"),
            "===" => format!("({left} === {right})"),
            "!==" => format!("({left} !== {right})"),
            "<" | ">" | "<=" | ">=" => format!("(KT.cmp({left}, {right}) {op} 0)"),
            "&&" => format!("(KT.truthy({left}) && KT.truthy({right}))"),
            "||" => format!("(KT.truthy({left}) || KT.truthy({right}))"),
            "in" => format!("KT.contains({right}, {left})"),
            "!in" => format!("(!KT.contains({right}, {left}))"),
            ".." => format!("KT.range({left}, {right})"),
            "..<" | "until" => format!("KT.until({left}, {right})"),
            "downTo" => format!("KT.range({left}, {right}, -1)"),
            "step" => format!("KT.step({left}, {right})"),
            "to" => format!("KT.pair({left}, {right})"),
            "and" => format!("KT.and({left}, {right})"),
            "or" => format!("KT.or({left}, {right})"),
            "xor" => format!("KT.xor({left}, {right})"),
            "shl" => format!("KT.shl({left}, {right})"),
            "shr" => format!("KT.shr({left}, {right})"),
            "ushr" => format!("KT.ushr({left}, {right})"),
            infix if self.known(infix) || self.cx.extensions.contains(infix) => {
                if self.cx.extensions.contains(infix) {
                    format!("(await self.ext({}, {left}, [{right}]))", quote(infix))
                } else {
                    format!("(await KT.m({left}, {}, [{right}]))", quote(infix))
                }
            }
            other => return Err(unsupported(format!("the operator {other}"))),
        })
    }

    // ---- Lambdas ----

    /// A lambda as an async function. `label` is the function it's passed to (`map`), which a
    /// `return@map` leaves.
    fn lambda(&mut self, lambda: &Lambda, label: &str) -> Result<String, String> {
        // The functions whose lambda takes nothing: an `it` in one is an outer lambda's.
        if lambda.implicit && NO_PARAMETER_LAMBDAS.contains(&label) {
            return self.lambda(&without_it(lambda), label);
        }
        let label = lambda.label.clone().unwrap_or_else(|| label.to_owned());
        self.scoped(|writer| {
            let mut params = Vec::new();
            let mut prologue = Vec::new();
            if lambda.implicit {
                params.push(writer.declare("it"));
            }
            for param in &lambda.params {
                match param {
                    // Unused parameters, each its own name (`{ _, _ -> }`).
                    LambdaParam::Name(name) if name == "_" => params.push(writer.temp()),
                    LambdaParam::Name(name) => params.push(writer.declare(name)),
                    LambdaParam::Destructured(names) => {
                        let temp = writer.temp();
                        params.push(temp.clone());
                        for (index, name) in names.iter().enumerate() {
                            if name != "_" {
                                let js = writer.declare(name);
                                prologue.push(format!("const {js} = KT.component({temp}, {index});"));
                            }
                        }
                    }
                }
            }
            writer.labels.push(label.clone());
            writer.depth += 1;
            let body = writer.block(&lambda.body, Tail::Return);
            writer.depth -= 1;
            writer.labels.pop();
            let body = body?;
            // A labelled return in an expression position comes as `KT.Return` with the label.
            let body = if label.is_empty() {
                body
            } else {
                format!(
                    "try {{ {body} }} catch (e) {{ if (e instanceof KT.Return && e.label === {}) return e.value; throw e; }}",
                    quote(&label)
                )
            };
            Ok(format!(
                "(async ({}) => {{ {} {body} }})",
                params.join(", "),
                prologue.join(" ")
            ))
        })
    }

    /// A lambda for a function type with a receiver (`HttpUrl.Builder.() -> Unit`): called with
    /// the receiver first, which its code reads implicitly.
    fn receiver_lambda(&mut self, lambda: &Lambda, label: &str) -> Result<String, String> {
        let receiver = self.temp();
        self.push_receiver(&receiver, [Some(label), lambda.label.as_deref()]);
        let function = self.lambda(&without_it(lambda), label);
        self.pop_receiver();
        Ok(format!(
            "(async ({receiver}, ...rest) => {}(...rest))",
            function?
        ))
    }

    /// A call's trailing lambda: with the builder as its receiver for kotlinx.serialization's
    /// JSON builders (`putJsonArray("tags") { add(tag) }`), else a plain one.
    fn trailing_lambda(&mut self, lambda: &Lambda, name: &str) -> Result<String, String> {
        if matches!(
            name,
            "putJsonArray" | "putJsonObject" | "addJsonArray" | "addJsonObject"
        ) || (self.cx.receiver_lambda_methods.contains(name) && !METHODS.contains(&name))
        {
            self.receiver_lambda(lambda, name)
        } else {
            self.lambda(lambda, name)
        }
    }

    /// A lambda called at once with no argument.
    fn lambda_call(&mut self, lambda: &Lambda, label: &str) -> Result<String, String> {
        let function = self.lambda(&without_it(lambda), label)?;
        Ok(format!("(await {function}())"))
    }

    /// A lambda called with `value` as `it` (or its parameter).
    fn it_call(&mut self, lambda: &Lambda, value: &str, label: &str) -> Result<String, String> {
        let function = self.lambda(lambda, label)?;
        Ok(format!("(await {function}({value}))"))
    }

    /// A lambda with `value` as its receiver (`run`, `with`), giving its own result.
    fn receiver_call(
        &mut self,
        lambda: &Lambda,
        value: &str,
        label: &str,
    ) -> Result<String, String> {
        let receiver = self.temp();
        self.push_receiver(&receiver, [Some(label), lambda.label.as_deref()]);
        let function = self.lambda(&without_it(lambda), label);
        self.pop_receiver();
        Ok(format!(
            "(await (async ({receiver}) => {}())({value}))",
            function?
        ))
    }

    /// A lambda with `value` as its receiver, giving the receiver (`apply`, `buildList`).
    fn receiver_call_returning(
        &mut self,
        lambda: &Lambda,
        value: &str,
        label: &str,
    ) -> Result<String, String> {
        let receiver = self.temp();
        self.push_receiver(&receiver, [Some(label), lambda.label.as_deref()]);
        let function = self.lambda(&without_it(lambda), label);
        self.pop_receiver();
        Ok(format!(
            "(await (async ({receiver}) => {{ await {}(); return {receiver}; }})({value}))",
            function?
        ))
    }
}

/// A lambda with a receiver (`apply { … }`, `buildString { … }`) has no `it` of its own: one in
/// it is an outer lambda's.
fn without_it(lambda: &Lambda) -> Lambda {
    Lambda {
        implicit: false,
        ..lambda.clone()
    }
}

/// Whether an expression can sit in a ternary as it is (no statements inside).
fn is_plain(expr: &Expr) -> bool {
    !matches!(
        expr,
        Expr::Return { .. } | Expr::Throw(_) | Expr::Break(_) | Expr::Continue(_)
    )
}

fn type_name(ty: &Type) -> String {
    if ty.args.is_empty() {
        ty.name.clone()
    } else {
        format!(
            "{}<{}>",
            ty.name,
            ty.args.iter().map(type_name).collect::<Vec<_>>().join(",")
        )
    }
}

/// A call's arguments as JavaScript's list, with the types it passes to reified type parameters
/// (`KT.typedArgs`), when it passes some.
fn typed_args(args: &[String], types: Option<&str>) -> String {
    match types {
        Some(types) => format!("KT.typedArgs([{}], {types})", args.join(", ")),
        None => format!("[{}]", args.join(", ")),
    }
}

/// The page a web view's block loads first (`loadUrl(checkUrl.toString())`), when it says.
fn loaded_page(block: &Block) -> Option<Expr> {
    block.iter().find_map(|stmt| match stmt {
        Stmt::Expr(Expr::Call { callee, args, .. }) => match callee.as_ref() {
            Expr::Name(name) if matches!(name.as_str(), "loadUrl" | "loadData") => {
                args.first().map(|arg| arg.value.clone())
            }
            _ => None,
        },
        Stmt::Expr(Expr::If {
            then, otherwise, ..
        }) => loaded_page(then).or_else(|| otherwise.as_ref().and_then(loaded_page)),
        _ => None,
    })
}

/// Whether a function's name says it passes a browser check (`passGuard`, `wafInterceptor`,
/// `solveTurnstile`), rather than reading a page a script draws.
fn passes_a_check(function: &str) -> bool {
    const MARKS: [&str; 16] = [
        "pass",
        "guard",
        "check",
        "challenge",
        "verify",
        "waf",
        "anubis",
        "bot",
        "clearance",
        "turnstile",
        "captcha",
        "gate",
        "cloudflare",
        "solve",
        "bypass",
        "shield",
    ];
    let lower = function.to_lowercase();
    MARKS.iter().any(|mark| lower.contains(mark))
}

/// Functions and objects the runtime has without a receiver (`mihon/kotlin.js`'s `GLOBALS`).
#[rustfmt::skip]
pub const GLOBALS: &[&str] = &[
    "Random", "StaticLayout",
    "listOf", "mutableListOf", "arrayListOf", "arrayOf", "listOfNotNull", "emptyList", "emptyArray",
    "setOf", "mutableSetOf", "hashSetOf", "linkedSetOf", "setOfNotNull", "emptySet", "mapOf",
    "mutableMapOf", "hashMapOf", "linkedMapOf", "emptyMap", "Pair", "Triple", "error", "require",
    "requireNotNull", "check", "checkNotNull", "TODO", "println", "print", "maxOf", "minOf",
    "Regex", "Exception", "RuntimeException", "IllegalStateException", "IllegalArgumentException",
    "UnsupportedOperationException", "NoSuchElementException", "IOException", "Error",
    "IndexOutOfBoundsException", "IntRange",
    "HttpException", "GET", "POST", "PUT", "DELETE", "MangasPage", "Page", "SMangaUpdate",
    "FilterList", "SimpleDateFormat", "StringBuilder", "Date", "JsonPrimitive", "JsonObject",
    "JsonArray", "Locale", "abs", "round", "floor", "ceil", "getPreferences", "getPreferencesLazy",
    "Selection", "Json", "Mutex", "Semaphore", "awaitAll", "Char", "String", "Intl", "Interceptor",
    "Comparator", "compareBy", "compareByDescending", "naturalOrder", "reverseOrder", "List",
    "MutableList", "Array", "arrayOfNulls", "HashMap", "LinkedHashMap", "ConcurrentHashMap", "HashSet",
    "LinkedHashSet", "DateTimeFormatterBuilder", "GraphQLErrorInterceptor", "graphQLBody",
    "persistedQueryExtension", "ArrayList", "ArrayDeque", "ByteArray", "IntArray", "LongArray",
    "DoubleArray", "FloatArray", "BooleanArray", "CharArray", "intArrayOf", "byteArrayOf",
    "longArrayOf", "doubleArrayOf", "floatArrayOf", "charArrayOf", "booleanArrayOf", "sequenceOf",
    "emptySequence", "min", "max", "synchronized", "CoroutineScope", "ratingStars", "aesCbc",
    "aesEcb", "aesGcm", "aesCtr", "newAesCipher", "IvParameterSpec", "SecretKeySpec",
    "GCMParameterSpec", "PBEKeySpec", "TextNode", "XorInterceptor", "Canvas", "Rect", "RectF",
    "Matrix", "Paint", "TextPaint", "Buffer", "ByteArrayOutputStream", "ByteArrayInputStream",
    "TextInterceptor",
    "GeneralSecurityException", "SecurityException", "UnknownHostException", "ProtocolException",
    "SocketTimeoutException", "InterruptedException", "NumberFormatException", "TimeoutException",
    "CancellationException", "generateSequence", "DecimalFormat", "SecureRandom", "Any",
    "LruCache", "CompletableDeferred", "URI", "URL", "SupervisorJob", "Job", "delay", "assert",
    "runWebView", "getTurnstileToken", "getLocalStorage", "SwitchPreferenceCompat",
    "CheckBoxPreference", "ListPreference", "EditTextPreference", "MultiSelectListPreference",
    "PreferenceCategory", "Cache", "SeedRandom", "showInfo", "askConfirm", "askInput",
    "askPassword", "askSelect", "askSelectOption", "GZIPInputStream", "InflaterInputStream",
    "SoftReference", "WeakReference", "randomUUID", "launch", "Header", "Separator",
    "injectLazy", "AtomicInteger", "AtomicLong", "AtomicBoolean", "AtomicReference",
    "ReentrantLock", "ReentrantReadWriteLock", "sleep", "AutoCloseable", "File", "Dispatcher",
    "PropertyDelegateProvider", "JSONObject", "JSONArray", "GZIPOutputStream",
    "DeflaterOutputStream", "BigInteger",
];

/// Objects the runtime has (`mihon/kotlin.js`'s `OBJECTS`): `Name.member`, or `Name` as a value.
#[rustfmt::skip]
pub const OBJECTS: &[&str] = &[
    "SManga", "SChapter", "Jsoup", "Fetch", "Locale", "TimeZone", "ZoneId", "ZoneOffset",
    "Calendar", "TimeUnit", "CacheControl", "UUID", "Json", "MediaType", "ResponseBody", "String", "Character",
    "Unit", "LocalDate", "LocalDateTime", "ZonedDateTime", "Parser", "DateTimeFormatter", "System",
    "FormBody", "Headers", "Request", "RegexOption", "Filter", "Math", "Integer", "Long", "Int",
    "URLEncoder", "URLDecoder", "Charsets", "Base64", "JsonNull", "Instant", "Clock", "AppInfo",
    "ChronoUnit", "ChronoField", "Log", "Regex", "Cookie", "CookieManager", "Random", "Build",
    "VERSION", "Normalizer", "Entities", "StandardCharsets", "Charset", "MarkdownDescription",
    "Cipher", "MessageDigest", "Mac", "SecretKeyFactory", "UpdateStrategy", "MultipartBody",
    "QuickJs", "JSON_MEDIA_TYPE", "Bitmap", "BitmapFactory", "Color", "Paint", "Typeface",
    "ByteBuffer", "ByteOrder", "ImageDecoder", "TextInterceptorHelper", "Collator", "Dispatchers",
    "Evaluator", "HttpUrl", "Uri", "Float", "Double", "Thread", "Response", "Protocol", "Year",
    "OffsetDateTime", "Resources", "MonthDay", "javaClass", "jsonInstance", "DayOfWeek",
    "TextStyle", "Injekt", "GlobalScope", "SSLContext", "TriState", "STATE_IGNORE",
    "STATE_INCLUDE", "STATE_EXCLUDE", "RequestBody", "CookieJar", "Credentials", "Html",
    "HttpURLConnection", "Duration", "DurationUnit", "CharCategory", "StaticLayout", "Layout",
    "LineBreaker", "Collections",
    "LazyThreadSafetyMode",
    "URLUtil", "Result",
    "FORCE_NETWORK", "FORCE_CACHE", "TimeSource", "RoundingMode", "Validate", "Runtime",
    "applicationContext", "PreferenceManager", "BuildConfig", "File", "Dns",
    "DEFAULT_BUFFER_SIZE", "InputType", "SignStyle", "FormatStyle", "YearMonth", "LocalTime",
    "WeekFields", "BigInteger",
];
