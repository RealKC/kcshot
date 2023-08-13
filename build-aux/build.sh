#!/usr/bin/env sh

echo were building
$1 build $2 && cp "src/$3" $4

echo $2
