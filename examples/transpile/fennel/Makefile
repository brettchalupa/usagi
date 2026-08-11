# Run `make` or `make all` to compile this folder's .fnl files into .lua files

# Find Fennel source files in the current folder
SOURCES = $(wildcard *.fnl)

# Via PATtern SUBSTitution, identify the would-be Lua files to generate
TARGETS = $(patsubst %.fnl,%.lua,$(SOURCES))

# Enumerate the target files to generate
all: $(TARGETS)

# Define a pattern rule that matches the target files
# For a given match, $< is the .fnl file (prerequisite), and $@ is the .lua file (target)
%.lua: %.fnl
	fennel --compile $< > $@
