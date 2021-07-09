#!/bin/bash

for file in "$@"; do
    fname="${file##*/}"
    fstem="${fname%.*}"
    echo "Processing $fname"
    sem '-j+4' 'exiftool -b -j '"${file@Q}" > "$fstem.json"
done
sem --wait
