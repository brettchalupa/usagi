-- Pure-Lua helpers attached to the `usagi` global. Loaded by
-- `setup_api` after the table is constructed; functions that don't
-- need a Rust closure live here so they're easy to read, fork, and
-- override at the script level.
-- NOTE: only `usagi.dump` is actually available from this file. The other
-- functions are helpers that it uses.

-- Render a table key. Bare-identifier keys stay unquoted for
-- readability; anything else (numbers, strings with punctuation,
-- etc.) gets the bracketed form so the output is unambiguous.
local function format_key(k)
  if type(k) == "string" and k:match("^[%a_][%w_]*$") then
    return k
  end
  return "[" .. tostring(k) .. "]"
end

-- Array-like = positive-integer keys exactly 1..n with no holes.
-- Returns (is_array, total_count). A mixed table or one with a
-- non-integer key always falls back to the map branch.
local function is_array_like(t)
  local n = 0
  for _ in pairs(t) do n = n + 1 end
  for i = 1, n do
    if t[i] == nil then return false, n end
  end
  return true, n
end

local function dump_value(v, indent, seen)
  local t = type(v)
  if t == "nil" then return "nil" end
  if t == "boolean" or t == "number" then return tostring(v) end
  -- %q quotes the string and escapes embedded quotes / newlines.
  if t == "string" then return string.format("%q", v) end
  if t == "function" then return "<function>" end
  if t == "userdata" then return "<userdata>" end
  if t == "thread" then return "<thread>" end
  if t == "table" then
    if seen[v] then return "<cycle>" end
    seen[v] = true
    local is_arr, n = is_array_like(v)
    if n == 0 then
      seen[v] = nil
      return "{}"
    end
    local indent2 = indent .. "  "
    local parts = {}
    if is_arr then
      for i = 1, n do
        parts[#parts + 1] = indent2 .. dump_value(v[i], indent2, seen)
      end
    else
      -- Sort keys for stable output: comparing tostring(k) handles
      -- mixed key types without errors.
      local keys = {}
      for k in pairs(v) do keys[#keys + 1] = k end
      table.sort(keys, function(a, b) return tostring(a) < tostring(b) end)
      for _, k in ipairs(keys) do
        parts[#parts + 1] = indent2 .. format_key(k) .. " = " .. dump_value(v[k], indent2, seen)
      end
    end
    seen[v] = nil
    return "{\n" .. table.concat(parts, ",\n") .. ",\n" .. indent .. "}"
  end
  return "<" .. t .. ">"
end

-- Pretty-prints any Lua value to a string. Tables are recursed with
-- sorted keys; arrays render in order; cycles render as `<cycle>`;
-- functions / userdata / threads render as placeholders. Pair with
-- `print(usagi.dump(state))` for terminal logging or feed the result
-- into `gfx.text` to draw it on screen during dev.
local function dump(v)
  return dump_value(v, "", {})
end

return dump
