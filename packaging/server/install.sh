#!/bin/sh
# Installs RadioMoon server as a service. Run as root from the unpacked folder.
set -e
[ "$(id -u)" = 0 ] || { echo "Run as root (sudo ./install.sh)"; exit 1; }
HERE=$(cd "$(dirname "$0")" && pwd)

echo "== Database"
wait_db() { # waits until MariaDB answers on 127.0.0.1:3306
    n=0
    while [ $n -lt 40 ]; do
        (mariadb-admin ping --silent 2>/dev/null || mysqladmin ping --silent 2>/dev/null) && return 0
        n=$((n+1)); sleep 1
    done
    return 1
}
if command -v apk >/dev/null 2>&1; then
    apk add --no-cache mariadb mariadb-client alsa-lib alsa-utils shadow
    [ -d /var/lib/mysql/mysql ] || mariadb-install-db --user=mysql --datadir=/var/lib/mysql
    # Alpine ships MariaDB with networking switched off: RadioMoon connects over 127.0.0.1
    mkdir -p /etc/my.cnf.d
    for f in /etc/my.cnf.d/*.cnf /etc/my.cnf; do
        [ -f "$f" ] && sed -i 's/^[[:space:]]*skip-networking/#skip-networking/' "$f"
    done
    printf '[mysqld]\nbind-address=127.0.0.1\nport=3306\n' > /etc/my.cnf.d/radiomoon.cnf
    rc-update add mariadb default || true
    rc-service mariadb restart || rc-service mariadb start || true
elif command -v apt-get >/dev/null 2>&1; then
    DEBIAN_FRONTEND=noninteractive apt-get install -y mariadb-server libasound2
    systemctl enable --now mariadb || systemctl enable --now mysql || true
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y mariadb-server alsa-lib
    systemctl enable --now mariadb || true
fi
wait_db || echo "MariaDB does not answer yet; continuing."

echo "== Program"
id radiomoon >/dev/null 2>&1 || {
    if command -v useradd >/dev/null 2>&1; then useradd --system --home-dir /var/lib/radiomoon --shell /usr/sbin/nologin radiomoon
    else adduser -S -h /var/lib/radiomoon -s /sbin/nologin radiomoon; fi
}
getent group audio >/dev/null 2>&1 && (usermod -aG audio radiomoon 2>/dev/null || addgroup radiomoon audio 2>/dev/null || true)
mkdir -p /opt/radiomoon /var/lib/radiomoon /etc/radiomoon
install -m 755 "$HERE/radiomoon-server" /opt/radiomoon/radiomoon-server
[ -d "$HERE/lang" ] && cp -r "$HERE/lang" /opt/radiomoon/
[ -f /etc/radiomoon/server.env ] || install -m 644 "$HERE/server.env" /etc/radiomoon/server.env
chown -R radiomoon:radiomoon /var/lib/radiomoon

echo "== Database for RadioMoon (user radiomoon, only from this computer)"
SQL="CREATE DATABASE IF NOT EXISTS radiomoon CHARACTER SET utf8mb4; CREATE USER IF NOT EXISTS 'radiomoon'@'127.0.0.1' IDENTIFIED BY 'radiomoon'; CREATE USER IF NOT EXISTS 'radiomoon'@'localhost' IDENTIFIED BY 'radiomoon'; GRANT ALL ON radiomoon.* TO 'radiomoon'@'127.0.0.1'; GRANT ALL ON radiomoon.* TO 'radiomoon'@'localhost'; FLUSH PRIVILEGES;"
mariadb -e "$SQL" 2>/dev/null || mysql -e "$SQL" || echo "Could not create the database automatically: create it by hand and run: /opt/radiomoon/radiomoon-server --setup --db-user ... --db-password ..."
su -s /bin/sh radiomoon -c "RADIOMOON_DATA=/var/lib/radiomoon /opt/radiomoon/radiomoon-server --setup --db-host 127.0.0.1 --db-port 3306 --db-user radiomoon --db-password radiomoon --db-name radiomoon" || echo "The server could not connect to the database (see above)."

if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    install -m 644 "$HERE/radiomoon-server.service" /etc/systemd/system/radiomoon-server.service
    systemctl daemon-reload
    systemctl enable --now radiomoon-server
    sleep 2
    systemctl --no-pager status radiomoon-server | head -5 || true
    echo
    echo "Password of the web studio:  grep password /var/lib/radiomoon/webserver.json"
elif command -v rc-service >/dev/null 2>&1; then
    install -m 755 "$HERE/radiomoon-server.openrc" /etc/init.d/radiomoon-server
    rc-update add radiomoon-server default
    rc-service radiomoon-server start
    echo "Password of the web studio:  grep password /var/lib/radiomoon/webserver.json"
else
    echo "No service manager found: start /opt/radiomoon/radiomoon-server by hand."
fi
echo "Open http://<this computer>:8080/ in a browser (user: admin)."
