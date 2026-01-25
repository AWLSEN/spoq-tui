#!/bin/bash
echo "Testing TTY environment..."
echo "stdin tty: $(test -t 0 && echo YES || echo NO)"
echo "stdout tty: $(test -t 1 && echo YES || echo NO)"
echo "stderr tty: $(test -t 2 && echo YES || echo NO)"
echo "TTY device: $(tty)"
echo "TERM: $TERM"
echo "Sleeping for 5 seconds..."
sleep 5
echo "Done"
