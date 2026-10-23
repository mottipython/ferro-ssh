FROM alpine:latest

# Install OpenSSH server, SFTP server, and tools
RUN apk add --no-cache openssh-server openssh-sftp-server bash shadow sudo curl

# Configure sshd for password authentication and SFTP
RUN ssh-keygen -A && \
    mkdir -p /var/run/sshd && \
    sed -i 's/#PasswordAuthentication yes/PasswordAuthentication yes/' /etc/ssh/sshd_config && \
    echo "PasswordAuthentication yes" >> /etc/ssh/sshd_config && \
    echo "PermitEmptyPasswords no" >> /etc/ssh/sshd_config

# Create test user with password authentication
# Username: testuser
# Password: password123
RUN useradd -m -s /bin/bash testuser && \
    echo "testuser:password123" | chpasswd && \
    echo "root:toor" | chpasswd && \
    echo "testuser ALL=(ALL) ALL" >> /etc/sudoers

# Create some sample files and directories in testuser's home for SFTP testing
RUN mkdir -p /home/testuser/documents /home/testuser/logs && \
    echo "Welcome to FerroSSH test server!" > /home/testuser/welcome.txt && \
    echo "Sample document content" > /home/testuser/documents/notes.txt && \
    chown -R testuser:testuser /home/testuser

EXPOSE 22

CMD ["/usr/sbin/sshd", "-D", "-e"]
