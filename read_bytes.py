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
