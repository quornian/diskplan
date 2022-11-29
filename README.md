```
.          .   0 .
 0   .      1   \  1 .
. \ 1  0. _/__0  \/ 0  ________________________________________________
 0_) \/  / 1  0  /_/ ____  ____  ___ __  __ ____  __     ____  __   __
    \/__(_/  (__/   .||  \  ||  ((   || //  || \\ ||    ||  || ||\  ||
     .  \  1.  \  0  ||   ) ||   \\  ||((   ||_// || __ ||__|| || \ ||
       1 \/    (_/   ||__/ _||_ __))_||_\\__||_  _||_||_||  ||_||_ \||_
        \_)   _)     __________________________________________________
           \_/
            (
            )
```

Diskplan
========

Diskplan is a command line tool and configuration system for constructing
directory trees from a set of schemas. It can:

- create files, directories and symlinks
- set owner, group, and UNIX permissions
- create directory entries with fixed names, or variable entries matching
  regular expressions
- define and reuse schema sub-trees
- with multiple roots, create and traverse symlinks into other stems

## Quickstart

To try out diskplan, a simple `diskplan.toml` file can be created:
```toml
[profiles.example]
root = "/tmp/diskplan-root"
schema = "examples/simple-schema.diskplan"
```
The associated schema file must also exist relative to the `diskplan.toml`
file:
```sh
# Root directory configuration
# ...
:let emptyfile = /dev/null

# Sub-directory
sub-directory/
    
    # Variable directory...
    $variable/
        # ...whose name must match this pattern:
        :match [A-Z][a-z]*

    # An empty file
    blank_file
        :source ${emptyfile}

```
We can now run diskplan in simulation mode (without `--apply`) to preview
the result:
```
$ diskplan /tmp/diskplan-root
[WARN  diskplan] Simulating in memory only, use --apply to apply to disk
[WARN  diskplan] Displaying in-memory filesystem...

[Root: /tmp/diskplan-root]
drwxr-xr-x root       root       /tmp/diskplan-root/
drwxr-xr-x root       root         sub-directory/
-rw-r--r-- root       root           blank_file
```

Sub-directories that match the schema may be created either by path or
variable assignment:
```
$ diskplan /tmp/diskplan-root/sub-directory/Example
$ diskplan /tmp/diskplan-root --vars 'variable:Example'
```
Both of these produce the following:
```
[Root: /tmp/diskplan-root]
drwxr-xr-x root       root       /tmp/diskplan-root/
drwxr-xr-x root       root         sub-directory/
drwxr-xr-x root       root           Example/
drwxr-xr-x root       root             inner-directory/
-rw-r--r-- root       root           blank_file
```

## Credits
- Ian Thompson (quornian)
- John van Leeuwen (fuglaro)
