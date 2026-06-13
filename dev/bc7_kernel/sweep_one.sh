#!/usr/bin/env bash
# args: ISPC_BIN TARGET OPTFLAGS LABEL
set -e
ISPC="$1"; TGT="$2"; OPT="$3"; LABEL="$4"
OBJ=/tmp/sweep_$LABEL.o
"$ISPC" --target=$TGT --pic $OPT -h /tmp/bc7e_ispc.h -o $OBJ /tmp/bc7enc_rdo/bc7e.ispc 2>/dev/null
gcc -O2 -fPIE -I/tmp -c /tmp/bc7kernel_harness.c -o /tmp/h_$LABEL.o
gcc /tmp/h_$LABEL.o $OBJ -o /tmp/hb_$LABEL -lm
echo "### $LABEL  target=$TGT opt=[$OPT]"
for perc in 1 0; do
  /tmp/hb_$LABEL /tmp/bc7_probe_s.bin $perc basic | grep -E 'RECOVERED|KEPT|sanity'
done
rm -f $OBJ /tmp/h_$LABEL.o /tmp/hb_$LABEL
