#!/usr/bin/env bash
# Deterministic desktop close-lifecycle gate (issue #40).
#
# Usage: scripts/test-desktop-close.sh <command...>
#   e.g. scripts/test-desktop-close.sh native/target/release/521c
#        scripts/test-desktop-close.sh ./native/dist/521C-0.1.0-x86_64.AppImage --appimage-extract-and-run
#
# Launches the app with --mock --close-self-test on the current (virtual)
# display. The app opens its window, then dispatches the same
# WindowEvent::CloseRequested a window manager's close button produces, and
# must exit cleanly within the timeout. A hang or non-zero exit means a
# normal window close would leave an invisible process behind.
#
# Requirements: a DISPLAY (use xvfb-run -a in headless environments).

set -euo pipefail

[ $# -ge 1 ] || { echo "usage: $0 <command...>" >&2; exit 2; }
[ -n "${DISPLAY:-}" ] || { echo "DISPLAY is not set; run under xvfb-run -a" >&2; exit 2; }

TIMEOUT="${CLOSE_TEST_TIMEOUT:-30}"

set +e
timeout "$TIMEOUT" "$@" --mock --close-self-test
code=$?
set -e

if [ "$code" -eq 0 ]; then
    echo "close-lifecycle OK: normal close ends the event loop and the process exits"
    exit 0
fi
if [ "$code" -eq 124 ]; then
    echo "FAIL: app did not exit within ${TIMEOUT}s after close (hidden survivor process)" >&2
else
    echo "FAIL: close self-test exited with status $code" >&2
fi
exit 1
