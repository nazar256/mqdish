#! /bin/bash

if [ -z "$1" ]; then
    echo "Usage: $0 <number_of_commands>"
    exit 1
fi  

for i in $(seq 1 $1); do
    echo "./dummy.sh $i" | target/debug/mqdish --topic mqdish-test
done
