#!/bin/bash
BINARY=$@
ssh $TEG_ARMV7_HOST "pkill mtree"
cat $BINARY | ssh $TEG_ARMV7_HOST "cat > ~/mtree && chmod 755 ~/mtree && RUST_BACKTRACE=full ~/mtree"
