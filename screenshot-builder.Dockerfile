FROM fedora:latest

RUN dnf install -y --setopt=install_weak_deps=False \
    xorg-x11-server-Xvfb \
    openbox \
    ImageMagick \
    xdotool \
    dejavu-sans-fonts \
    mesa-dri-drivers \
    libX11 \
    libXcursor \
    libXrandr \
    libxkbcommon-x11 \
    openssh-clients \
    procps-ng \
    && dnf clean all

WORKDIR /app
