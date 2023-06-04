#! /bin/sh

targetdir="$1"
if [ -z $targetdir ] ; then
  echo "Usage: $0 <targetdir>"
  exit 1
fi

for file in *.wav ; do
  raw=${targetdir}/$(basename ${file} .wav).raw
  echo "${file} => ${raw}"
  sox ${file} -b 8 -e unsigned-integer -c 1 -r 16000 ${raw}
done
