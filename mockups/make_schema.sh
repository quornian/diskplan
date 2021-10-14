#!/bin/bash
set -euo pipefail
tree=/tmp/schema-tree-test
rm -rf "$tree"
mkdir "$tree"
cd "$tree"

ln -s   '{$PL_SHOW}'        _.let.@show
ln -s   '/net/{@show}-tx'   _.let.@txdisk
mkdir                       _.def.@emptyfile
touch                       _.def.@emptyfile/_.is.file
mkdir                       asset
ln -s   showadmin           asset/_.group
mkdir                       asset/.division
ln -s   @emptyfile          asset/.division/_.is.reuse
mkdir                       asset/@category
ln -s   @category           asset/@another
ln -s   '[a-z]+'            asset/@category/_.match
ln -s   showadmin           asset/@category/_.group
ln -s   0o755               asset/@category/_.perms
touch                       asset/@category/.assetgroup
mkdir                       asset/@category/@asset
touch                       asset/@category/@asset/.asset
ln -s   '[A-Z][a-z]+'       asset/@category/@asset/_.match
ln -s   showadmin           asset/@category/@asset/_.group
ln -s   0o755               asset/@category/@asset/_.perms
mkdir                       asset/@category/@asset/texture
ln -s   '{@txdisk}/{PATH}'  asset/@category/@asset/texture/_.is.link

tree -Ua
#root_4
#└── asset
#    ├── @category
#    │   ├── @asset
#    │   │   ├── .asset
#    │   │   │   ├── _.content
#    │   │   │   └── _.perms -> rw-------
#    │   │   ├── _.group -> showadmin
#    │   │   ├── _.namerule -> [A-Z][A-Za-z]+
#    │   │   ├── _.perms -> rwxr-xr-x
#    │   │   └── texture
#    │   │       └── _.content -> {@txdisk}/{PATH}
#    │   ├── .assetgroup
#    │   │   ├── _.content
#    │   │   └── _.perms -> rw-------
#    │   ├── _.group -> showadmin
#    │   ├── _.namerule -> [a-z]+
#    │   └── _.perms -> rwxr-xr-x
#    ├── .division
#    │   ├── _.content
#    │   └── _.perms -> r--r--r--
#    ├── _.group -> showadmin
#    └── _.perms -> rwxr-xr-x
#
#7 directories, 15 files
