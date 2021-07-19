#!/bin/bash

#### WARNING: DEPRECATED
#### Use `-x` option of the transform binary (much faster in parallel than sem)

for file in "$@"; do
    fname="${file##*/}"
    fstem="${fname%.*}"
    echo "$fstem"
    sem --id copy-exif '-j' '40' 'exiv2 -ea- '"${file@Q}"' | exiv2 -ia- '"$fstem.tif"
    sem --id copy-exif '-j' '40' 'exiv2 -eX- '"${file@Q}"' | exiv2 -iX- '"$fstem.tif"

done
sem --id copy-exif --wait
