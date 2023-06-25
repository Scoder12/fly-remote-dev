FROM rust:1.66.0-slim-bullseye AS builder

WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/app/target \
		--mount=type=cache,target=/usr/local/cargo/registry \
		--mount=type=cache,target=/usr/local/cargo/git \
		--mount=type=cache,target=/usr/local/rustup \
		set -eux; \
		rustup install stable; \
	 	cargo build --release; \
		objcopy --compress-debug-sections target/release/remote-dev ./remote-dev

################################################################################
FROM ubuntu:20.04

RUN set -eux; \
		export DEBIAN_FRONTEND=noninteractive; \
	  apt update; \
		apt install --yes --no-install-recommends \
			bind9-dnsutils iputils-ping iproute2 curl ca-certificates htop \
			curl wget ca-certificates git-core \
			openssh-server openssh-client \
			sudo less zsh build-essential \
			; \
		apt clean autoclean; \
		apt autoremove --yes; \
		rm -rf /var/lib/{apt,dpkg,cache,log}/; \
		echo "Installed base utils!"

RUN set -eux; \
		useradd -ms /usr/bin/zsh spencer; \
		usermod -aG sudo spencer; \
		echo '%sudo ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers; \
		echo "added user"

RUN set -eux; \
		echo "Port 22" >> /etc/ssh/sshd_config; \
		echo "AddressFamily inet" >> /etc/ssh/sshd_config; \
		echo "ListenAddress 0.0.0.0" >> /etc/ssh/sshd_config; \
		echo "PasswordAuthentication no" >> /etc/ssh/sshd_config; \
		echo "ClientAliveInterval 30" >> /etc/ssh/sshd_config; \
		echo "ClientAliveCountMax 10" >> /etc/ssh/sshd_config; \
		echo "SSH server set up"

WORKDIR app
COPY --from=builder /app/remote-dev ./remote-dev

RUN curl -fsSL https://code-server.dev/install.sh | sh
RUN grep -rl "style-src 'self' 'unsafe-inline'" /usr/lib/code-server \
	| sudo xargs sed -i \
		"s/style-src 'self' 'unsafe-inline'/style-src 'self' 'unsafe-inline' fonts.googleapis.com/g" && \
	grep -rl "font-src 'self' blob:" /usr/lib/code-server \
	| sudo xargs sed -i \
		"s/font-src 'self' blob:/font-src 'self' blob: fonts.gstatic.com/g"

USER spencer

# cannot install extensions/configuration here as home volume will mount over them

ENV RUST_BACKTRACE=full
CMD ["./remote-dev"]
