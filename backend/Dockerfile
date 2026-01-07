# 编译层
FROM rust:1.83-slim-bookworm AS build-env

WORKDIR /app

# 安装编译系统依赖
RUN apt-get update && apt-get install -y pkg-config libssl-dev musl-tools

# 复制依赖文件
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# 编译 release 版本
RUN cargo build --release

# 运行时层
FROM debian:bookworm-slim

# 安装运行时依赖和 PrinceXML 依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    openssl \
    wget \
    fontconfig \
    libfreetype6 \
    libxml2 \
    libgomp1 \
    libbrotli1 \
    fonts-noto-cjk \
    fonts-wqy-microhei \
    fonts-wqy-zenhei \
    && rm -rf /var/lib/apt/lists/*

# 下载安装 PrinceXML
RUN wget https://www.princexml.com/download/prince-15.3-linux-generic-x86_64.tar.gz && \
    tar -xzf prince-15.3-linux-generic-x86_64.tar.gz && \
    cd prince-15.3-linux-generic-x86_64 && \
    ./install.sh /usr/local && \
    cd .. && \
    rm -rf prince-15.3-linux-generic-x86_64*

WORKDIR /app

# 复制二进制文件
COPY --from=build-env /app/target/release/network_insight_backend ./backend

# 非 root 用户
RUN useradd -m appuser
RUN mkdir logs && chown -R appuser:appuser /app
USER appuser

EXPOSE 3001

ENV RUST_LOG=info
# Prince 默认安装在 /usr/local/bin/prince
ENV PRINCE_PATH=/usr/local/bin/prince

ENTRYPOINT ["./backend"]
