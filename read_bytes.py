# Example usage:
# ```sh
# socat TCP4-LISTEN:1234 - | python3 read_bytes.py
# ```

import os
import struct

id = os.read(0, 4)
(id,) = struct.unpack("!i", id)
print(f"ID: {id}")

while True:
    bytes = os.read(0, 2)
    (count,) = struct.unpack("!h", bytes)
    if count > 0:
        out = "+" * count
    elif count < 0:
        out = "-" * -count
    else:
        out = "."
    print(out, end=None)
