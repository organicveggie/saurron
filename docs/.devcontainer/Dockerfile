ARG RUBY_VERSION=4
FROM ruby:$RUBY_VERSION-slim

ENV SETUPDIR=/setup
WORKDIR ${SETUPDIR}
ARG GEMFILE_DIR=.
COPY $GEMFILE_DIR/packages* ./

RUN <<RUN_CMD_EOF 
set -eux

apt update
apt install -y --no-install-recommends \
    build-essential \
    zlib1g-dev

# Install Bundler
gem install bundler

# Install extra packages if needed
if [ -e packages ]; then
    apt install -y --no-install-recommends $(cat packages)
fi

# Cleanup
apt-get clean -y
rm -rf /var/lib/apt/lists/*
rm -rf \
    ${SETUPDIR} \
    /usr/gem/cache \
    /root/.bundle/cache
RUN_CMD_EOF

EXPOSE 4000
