# BSD Filer v0.3 (Dai Ishijima, 1996), a Unix emulation of the Filer
# server of the HP 100LX/200LX, with its serial side bridged to TCP port
# 4853 by socat. A black-box oracle for filer-proto and `satx lx`
# (kb/docs/clean-room-rule.md): the build fetches the author's published
# archive and compiles it here; its source is never copied into the
# repository, and the image is local only, never pushed.
# Build:  docker build -t filer-oracle tools/filer-oracle
# Run:    docker run -d -p 4853:4853 --name lx filer-oracle
# Then:   satx lx --port tcp://localhost:4853 ls

# The base image pinned by digest, the same as tools/saturnng-oracle's.
FROM debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2 AS build
# The author's page lists v0.3 as bsdfil03.lzh; the checksum is that of
# the archive fetched on 2026-11-08.
ARG BSDFILER_URL=http://vvvvvv.wheel.gr.jp/~dai/software/bsdfil03.lzh
ARG BSDFILER_SHA256=fdca9760ae4fd34c26b9512de5a970e541fe9181ab55862132b41f2e1cfa3404
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      gcc libc6-dev curl ca-certificates libarchive-tools \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
# LHa archive with CR/LF line ends; libarchive's bsdtar reads it. The C
# files compile as they are in GNU C89 mode (kb/research/hp100lx-filer.md,
# "Testing without the owner's device").
RUN curl -fsSL "$BSDFILER_URL" -o bsdfil03.lzh \
    && echo "$BSDFILER_SHA256  bsdfil03.lzh" | sha256sum -c - \
    && bsdtar -xf bsdfil03.lzh \
    && dir=$(dirname "$(find . -name '*.c' | head -n 1)") \
    && cd "$dir" \
    && sed -i 's/\r$//' ./*.c ./*.h \
    && gcc -std=gnu89 -w -O2 -include sys/vfs.h -o /usr/local/bin/bsdfiler ./*.c \
    && test -x /usr/local/bin/bsdfiler

FROM debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      socat procps \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /usr/local/bin/bsdfiler /usr/local/bin/bsdfiler
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
COPY serve-one.sh /usr/local/bin/serve-one
RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/serve-one
ENV SPEED=19200
EXPOSE 4853
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
