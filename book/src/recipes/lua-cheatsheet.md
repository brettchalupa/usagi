# Lua Cheatsheet

## Variables

Variables allow you to keep track of data that changes over time.

```lua
health = 20
```

Prefer using `local` when possible so you don't accidentally overwrite a global:

```lua
local health = 20
```

### Variable Types

Basic types:

```lua
local nothing = nil                    -- nil
local is_alive = true                   -- boolean
local health = 100                     -- number
local name = "Player"                  -- string
local update = function() end          -- function
local data = {}                        -- table
```

### Multiple Assignment

```lua
local x, y = 10, 20
x, y = y, x  -- swap values
```

## Functions

Functions allow you to encapsulate and reuse code.

```lua
function damage_player(amount)
  health -= amount
end
```

### Local Functions

```lua
local function calculate_damage(base, modifier)
  return base * modifier
end
```

### Anonymous Functions

```lua
local on_click = function()
  print("Button clicked!")
end
```

### Functions with Multiple Returns

```lua
function get_position()
  return x, y
end

local player_x, player_y = get_position()
```

### Variable Arguments

```lua
function print_all(...)
  local args = {...}
  for i = 1, #args do
    print(args[i])
  end
end

print_all("hello", "world", 123)
```

## Tables

Tables are Lua's primary data structure for collections.

### Create Tables

```lua
local empty = {}
local numbers = {1, 2, 3, 4, 5}
local player = {
  x = 100,
  y = 50,
  health = 20
}
```

### Access Table Values

```lua
-- Array-style (numeric indices, starts at 1)
local first = numbers[1]  -- 1

-- Dictionary-style (string keys)
local health = player.health      -- dot notation
local health = player["health"]   -- bracket notation
```

### Add to a Table

```lua
-- Append to array
table.insert(numbers, 6)
numbers[#numbers + 1] = 7

-- Add properties
player.score = 0
player["level"] = 1
```

### Remove from Table

```lua
-- Remove by index
table.remove(numbers, 1)  -- removes first element
table.remove(numbers)     -- removes last element

-- Remove by key
player.health = nil
```

### Iterate Through Tables

```lua
-- Array iteration (numeric indices)
for i = 1, #numbers do
  print(numbers[i])
end

-- Generic iteration (all key-value pairs)
for key, value in pairs(player) do
  print(key, value)
end

-- Ordered iteration (numeric indices only)
for index, value in ipairs(numbers) do
  print(index, value)
end
```

### Table Length

```lua
local count = #numbers      -- length of array part
local count = #"hello"      -- string length (5)
```

## Strings

### String Creation

```lua
local message = "Hello, world!"
local multiline = [[
This is a
multi-line string
]]
```

### String Concatenation

```lua
local greeting = "Hello, " .. name .. "!"
```

### String Methods

```lua
local text = "Playdate"

text:upper()           -- "PLAYDATE"
text:lower()           -- "playdate"
text:len()             -- 8
text:sub(1, 4)         -- "Play"
text:find("date")      -- 5
text:gsub("Play", "X") -- "Xdate"
```

### String Formatting

```lua
local message = string.format("Score: %d, Time: %.2f", score, time)
```

### String Patterns

```lua
-- Playdate supports Lua's pattern matching
local result = string.match("hello world", "(%w+)")  -- "hello"
local newStr = string.gsub("hello world", "world", "Playdate")  -- "hello Playdate"
```

## Control Flow

### If Statements

```lua
if health > 0 then
  print("Player is alive")
elseif health == 0 then
  print("Player is unconscious")
else
  print("Player is dead")
end
```

### Logical Operators

```lua
-- and, or, not
if health > 0 and mana > 10 then
  castSpell()
end

-- Short-circuit evaluation
local value = input or "default"  -- use "default" if input is nil/false
```

### Loops

```lua
-- While loop
while health > 0 do
  update()
end

-- For loop (numeric)
for i = 1, 10 do
  print(i)
end

for i = 10, 1, -1 do  -- countdown
  print(i)
end

-- For loop (generic)
for key, value in pairs(player) do
  print(key, value)
end

-- Break out of a loop
for i = 1, 100 do
  if i == 50 then
    break  -- exit loop
  end
  -- Note: Lua doesn't have 'continue', use if-else instead
end
```

## Operators

### Arithmetic

```lua
local a, b = 10, 3

a + b   -- 13 (addition)
a - b   -- 7  (subtraction)
a * b   -- 30 (multiplication)
a / b   -- 3.333... (division)
a // b  -- 3  (floor division)
a % b   -- 1  (modulo)
a ^ b   -- 1000 (exponentiation)
```

### Comparison

```lua
a == b  -- false (equal)
a ~= b  -- true  (not equal)
a < b   -- false (less than)
a <= b  -- false (less than or equal)
a > b   -- true  (greater than)
a >= b  -- true  (greater than or equal)
```

### Assignment Shortcuts

These are Usagi Lua extensions, not part of standard Lua 5.5:

```lua
health += 10   -- health = health + 10
health -= 5    -- health = health - 5
health *= 2    -- health = health * 2
health /= 4    -- health = health / 4
```

## Common Patterns

### Default Values

```lua
function greet(name)
  name = name or "Player"  -- default value
  print("Hello, " .. name)
end
```

### Module Pattern

```lua
-- mymodule.lua
local M = {}

function M.do_something()
  print("Module function called")
end

return M
```

```lua
-- main.lua
local mymodule = require("mymodule")
mymodule.do_something()
```

Imported files can also just define globals directly (no `return` needed) and
skip capturing the return value: `require("mymodule")`.

## Built-in Math Functions

```lua
math.abs(-5)           -- 5
math.min(1, 2, 3)      -- 1
math.max(1, 2, 3)      -- 3
math.floor(3.7)        -- 3
math.ceil(3.2)         -- 4
math.random()          -- random float between 0 and 1
math.random(6)         -- random integer between 1 and 6
math.random(10, 20)    -- random integer between 10 and 20
math.sqrt(16)          -- 4
math.sin(math.pi/2)    -- 1
math.cos(0)            -- 1
math.rad(180)          -- converts degrees to radians
math.deg(math.pi)      -- converts radians to degrees
```

## Usagi Cheatsheet

[View Usagi's cheatsheet.](https://usagiengine.com/#cheatsheet)
