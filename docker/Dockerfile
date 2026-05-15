FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
        coreutils \
        curl \
        gh \
        git \
        jq \
        make \
        patch \
        python3 \
        python3-pip \
        ripgrep \
        sed \
        tar \
        tzdata \
        unzip \
        xz-utils \
        zip \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

ENV HOME=/workspace

CMD ["/bin/sh"]
