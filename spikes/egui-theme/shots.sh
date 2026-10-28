#!/usr/bin/env bash
# Build the spike and take the comparison screenshots.
#   shots.sh <out-dir> [regular-font bold-font]
# Run inside the repo's nix-shell. Two workarounds for the shell on the author's machine, both optional:
#   RUSTC_WRAPPER= is exported (sccache fails on the nix-shell temp dir);
#   GLIBC=<store path> re-points a copy of the binary at that glibc (see FINDINGS.md).
set -euo pipefail
out=$(realpath -m "${1:?usage: shots.sh <out-dir> [regular-font bold-font]}")
cd "$(dirname "$0")"
export RUSTC_WRAPPER=
cargo build 2>&1 | grep -E '^(error|warning)' -A8 || true
mkdir -p "$out"
bin="$out/spike-bin"
cp target/debug/egui-theme-spike "$bin"
if [ -n "${GLIBC:-}" ]; then
  patchelf --set-interpreter "$GLIBC/lib/ld-linux-x86-64.so.2" --set-rpath "$GLIBC/lib:$(patchelf --print-rpath "$bin")" "$bin"
fi
[ -n "${2:-}" ] && export SPIKE_FONT=$2 SPIKE_FONT_BOLD=${3:?bold font path}
for v in "light:" "dark:" "light:SPIKE_DIALOG=1" "dark:SPIKE_DIALOG=1" "light:SPIKE_FOCUS=1" "light:SPIKE_DATEPICKER=1" "dark:SPIKE_DATEPICKER=1"; do
  theme=${v%%:*}; knob=${v#*:}; name=$theme${knob:+_${knob%%=*}}
  env SPIKE_THEME=$theme ${knob:+$knob} SPIKE_SHOT="$out/$name.png" timeout 60 "$bin" 2>&1 | tail -3
done
rm -f "$bin"
ls "$out"
