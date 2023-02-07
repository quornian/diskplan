```text
.          .   0 .          .                      .
 0   .      1   \  1   .            .
. \ 1  0. _/_0   \/ 0 ________________________________________
 0_) \/  / 1  0  /_/  ____  ___  __ __  _ _ __  _     __  _  _
   .\/__(_/  (__/   .  |  \  |  (_   |_/   |__) |   .|__| |\ |
       0 \/  . (_/.   _|__/ _|_.__) _| \_ _|_   |__) |  | | \|
      . \_)   _) 1    ________________________________________
         . \_/__/     .                          .
            (/                            .            .
            )              .
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
- with multiple rooted stems, create and traverse symlinks into other stems

## Quickstart

The examples expect `diskplan` to be an available command. If you have the
code checked out, you can do something like this to make it available in the
current terminal:

```sh
$ cargo build
$ export PATH="$PWD/target/debug:$PATH"
```

Proper installation is left to the reader at present.

To run diskplan with a very quick example (and no changes to disk), run:

```sh
$ cd examples/quickstart
$ diskplan /tmp/diskplan-root
```

You'll be shown the following preview:

```text
[WARN  diskplan] Simulating in memory only, use --apply to apply to disk
[WARN  diskplan] Displaying in-memory filesystem...

[Root: /tmp/diskplan-root]
drwxr-xr-x root       root       /tmp/diskplan-root/
drwxr-xr-x root       root         sub-directory/
-rw-r--r-- root       root           blank_file
```

Diskplan looks in the current directory for a `diskplan.toml` file. Here are
the contents of that file for this example:

```toml
[stems.main]
root = "/tmp/diskplan-root"
schema = "simple-schema.diskplan"
```

The "main" stem associates a root path on disk (inside which construction will
be contained) with a schema to apply to paths within this root. The schema file
is found relative to the config and for this example contains the following:

```sh
# Root directory configuration
# ...
:let emptyfile = /dev/null

# Sub-directory
sub-directory/

    # Variable directory...
    $variable/
        # ...whose name must match this pattern...
        :match [A-Z][a-z]*

        # ...will then create this
        inner-directory/

    # An empty file
    blank_file
        :source ${emptyfile}
```

Note that in the earlier output, the `sub-directory` and `blank_file` were
created, but nothing for `$variable`. This variable directory can be created
either directly either by path or by assigning a value to this variable:

```text
$ diskplan /tmp/diskplan-root/sub-directory/Example
$ diskplan /tmp/diskplan-root --vars 'variable:Example'
```

Both of these produce the following output:

```text
[Root: /tmp/diskplan-root]
drwxr-xr-x root       root       /tmp/diskplan-root/
drwxr-xr-x root       root         sub-directory/
drwxr-xr-x root       root           Example/
drwxr-xr-x root       root             inner-directory/
-rw-r--r-- root       root           blank_file
```
