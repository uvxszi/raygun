-- Shosetsu's Lua globals (M32), run before every Shosetsu script.
--
-- The helpers (map, filter, first, flatten, mapNotNil, map2flat, pipeline, wrap, GET, POST,
-- pageOfElem, NodeVisitor, NovelInfo, NovelChapter, Novel, NovelStatus, ChapterType) are adapted
-- from shosetsuorg/kotlin-lib's Lua resources (GPL-3.0). The objects that kotlin-lib makes in
-- Kotlin (novels, chapters, filters, listings) are plain Lua tables here.

QUERY = 0
PAGE = 1
KEY_NOVEL_URL = 1
KEY_CHAPTER_URL = 2

local function capitalised(name)
    return name:sub(1, 1):upper() .. name:sub(2)
end

-- A class whose objects have a setX/getX pair per field.
local function record(kind, fields)
    local methods = {}
    for _, field in ipairs(fields) do
        methods["set" .. capitalised(field)] = function(self, value)
            self[field] = value
            return self
        end
        methods["get" .. capitalised(field)] = function(self)
            return self[field]
        end
    end
    local meta = { __index = methods }
    return function()
        return setmetatable({ __kind = kind }, meta)
    end
end

local novelFields = {
    "title", "alternativeTitles", "link", "imageURL", "language", "description", "status",
    "tags", "genres", "authors", "artists", "chapters", "chapterCount", "wordCount",
    "commentCount", "viewCount", "favoriteCount",
}
local chapterFields = { "release", "title", "link", "order", "sourceId" }

_NovelInfo = record("novel", novelFields)
_Novel = _NovelInfo
_NovelChapter = record("chapter", chapterFields)
_NovelStatus = function(value) return value end
_ChapterType = function(value) return value end

local function filled(make, fields)
    return function(values)
        local object = make()
        if values then
            for _, field in ipairs(fields) do
                if values[field] ~= nil then object[field] = values[field] end
            end
        end
        return object
    end
end

NovelInfo = filled(_NovelInfo, novelFields)
Novel = NovelInfo
NovelChapter = filled(_NovelChapter, chapterFields)

local function enum(name, values, make)
    return setmetatable({}, {
        __call = function(_, value) return make(value) end,
        __index = function(self, key)
            if type(key) ~= "string" or values[key] == nil then
                error(("Invalid %s type %s"):format(name, tostring(key)), 2)
            end
            return self(values[key])
        end,
    })
end

NovelStatus = enum("NovelStatus", { PUBLISHING = 0, COMPLETED = 1, PAUSED = 2, UNKNOWN = -1 }, _NovelStatus)
ChapterType = enum("ChapterType", { STRING = 0, HTML = 1, EPUB = 2, PDF = 3, MARKDOWN = 4 }, _ChapterType)

function Listing(name, increments, func)
    return { __kind = "listing", name = name, increments = increments, func = func }
end

-- Filters: tables the app reads back for their ids and default values.
local function makeFilter(kind, id, name, extra)
    local object = { __kind = "filter", kind = kind, id = id, name = name }
    for key, value in pairs(extra or {}) do object[key] = value end
    return object
end

function TextFilter(id, name) return makeFilter("text", id, name) end
function PasswordFilter(id, name) return makeFilter("password", id, name) end
function SwitchFilter(id, name) return makeFilter("switch", id, name) end
function CheckboxFilter(id, name) return makeFilter("checkbox", id, name) end
function TriStateFilter(id, name) return makeFilter("tristate", id, name) end
function DropdownFilter(id, name, choices) return makeFilter("dropdown", id, name, { choices = choices }) end
function RadioGroupFilter(id, name, choices) return makeFilter("radio", id, name, { choices = choices }) end
function FilterList(name, filters) return makeFilter("list", nil, name, { filters = filters }) end
function FilterGroup(name, filters) return makeFilter("group", nil, name, { filters = filters }) end
function HeaderFilter(name) return makeFilter("header", nil, name) end
function SeparatorFilter() return makeFilter("separator", nil, "") end

function AsList(value) return value end

function Reverse(list)
    local count = #list
    for index = 1, math.floor(count / 2) do
        list[index], list[count - index + 1] = list[count - index + 1], list[index]
    end
    return list
end

local function unsupported(name)
    return function() error(name .. " isn't supported in Raygun", 2) end
end

HttpClient = unsupported("HttpClient")
HttpClientBuilder = unsupported("HttpClientBuilder")
CookieJar = unsupported("CookieJar")
CookieBuilder = unsupported("CookieBuilder")
Interceptor = unsupported("Interceptor")

function GET(url, headers, cache)
    return _GET(url, headers or DEFAULT_HEADERS(), cache or DEFAULT_CACHE_CONTROL())
end

function POST(url, headers, body, cache)
    return _POST(url, headers or DEFAULT_HEADERS(), body or DEFAULT_BODY(), cache or DEFAULT_CACHE_CONTROL())
end

function NodeVisitor(head, tail, elementsOnly)
    if elementsOnly == nil then elementsOnly = true end
    return { head = head, tail = tail, elementsOnly = elementsOnly }
end

function pageOfElem(node, removeStyle, customStyle, keepScripts)
    node = node or error("No element for pageOfElem")
    return _PageOfElem(node, removeStyle == true, customStyle or "", keepScripts == true)
end

function wrap(object, func)
    return function(...) return func(object, ...) end
end

local function each(list, visit)
    if type(list) == "table" then
        for key, value in pairs(list) do visit(value, key) end
    else
        for index = 0, list:size() - 1 do visit(list:get(index), index) end
    end
end

function map(list, func)
    local result = {}
    if type(list) == "table" then
        for key, value in pairs(list) do result[key] = func(value, key) end
    else
        for index = 0, list:size() - 1 do result[index + 1] = func(list:get(index), index) end
    end
    return result
end

function mapNotNil(list, func)
    local result, count = {}, 0
    each(list, function(value)
        local mapped = func(value)
        if mapped then
            count = count + 1
            result[count] = mapped
        end
    end)
    return result
end

function filter(list, func)
    local result, count = {}, 0
    each(list, function(value, key)
        if func(value, key) then
            count = count + 1
            result[count] = value
        end
    end)
    return result
end

function first(list, func)
    if type(list) == "table" then
        for _, value in pairs(list) do
            if func(value) then return value end
        end
        return nil
    end
    for index = 0, list:size() - 1 do
        local value = list:get(index)
        if func(value) then return value end
    end
    return nil
end

function flatten(lists)
    local result, count = {}, 0
    for _, list in pairs(lists) do
        each(list, function(value)
            count = count + 1
            result[count] = value
        end)
    end
    return result
end

function map2flat(list, first, second)
    return flatten(map(map(list, first), function(inner) return map(inner, second) end))
end

function pipeline(object)
    return function(func, ...)
        if not func then return object end
        return pipeline(func(object, ...))
    end
end
