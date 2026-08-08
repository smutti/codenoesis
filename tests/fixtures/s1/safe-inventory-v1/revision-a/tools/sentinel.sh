#!/bin/sh
set -eu
printf 'executed\n' > "${CODENOESIS_SENTINEL:?}"
