FROM python:3.12-bookworm AS python-runtime

FROM node:24-bookworm

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PIP_DISABLE_PIP_VERSION_CHECK=1 \
    PYTHONDONTWRITEBYTECODE=1

RUN apt-get update && apt-get install --no-install-recommends -y \
    build-essential \
    curl \
    git \
    libayatana-appindicator3-dev \
    libgtk-3-dev \
    libssl-dev \
    libwebkit2gtk-4.1-dev \
    patchelf \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Keep the pinned Python runtime from the official 3.12 image while using the
# official Node 24 image as the primary base. Both images use Debian Bookworm.
COPY --from=python-runtime /usr/local /usr/local

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain 1.97.0 --profile minimal \
    && rustup component add rustfmt clippy --toolchain 1.97.0

ENV PATH="/usr/local/cargo/bin:${PATH}"

WORKDIR /workspace
COPY package.json package-lock.json ./
COPY apps/desktop/package.json apps/desktop/package.json
COPY services/agent/package.json services/agent/package.json
COPY services/broker/package.json services/broker/package.json
COPY services/orchestrator/package.json services/orchestrator/package.json
COPY services/editor-adapter/package.json services/editor-adapter/package.json
RUN npm ci

COPY requirements-dev.txt pyproject.toml .python-version ./
COPY services/mlhost/requirements.txt services/mlhost/requirements.txt
RUN python3.12 -m venv /opt/custodian-venv \
    && /opt/custodian-venv/bin/pip install --no-cache-dir \
       -r services/mlhost/requirements.txt -r requirements-dev.txt
ENV PATH="/opt/custodian-venv/bin:${PATH}"

COPY . .

# The container is a validation/build environment. It does not provide a
# Windows desktop session or claim to replace Windows installer acceptance.
CMD ["make", "test"]
