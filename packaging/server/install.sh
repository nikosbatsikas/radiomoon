#!/bin/sh
# RadioMoon server: install, upgrade or uninstall. Run as root from the unpacked folder.
#   ./install.sh                    install (or upgrade when it is already installed)
#   ./install.sh upgrade            replace the program, keep the settings, the database and the songs
#   ./install.sh uninstall          remove the program and the service; keep the data folder and the database
#   ./install.sh uninstall --purge  remove everything, the database and the uploaded songs too (asks first; -y does not ask)
#   RADIOMOON_REMOTE_DB=1 ./install.sh   (not needed by the RadioMoon client any more: it uses the web port of the server)
set -e
[ "$(id -u)" = 0 ] || { echo "Run as root (sudo ./install.sh)"; exit 1; }
HERE=$(cd "$(dirname "$0")" && pwd)
MODE=${1:-install}
PURGE=0; YES=0
for a in "$@"; do
    [ "$a" = "--purge" ] && PURGE=1
    [ "$a" = "-y" ] && YES=1
done

have_systemd() { command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; }
svc_stop() {
    if have_systemd; then systemctl stop radiomoon-server 2>/dev/null || true
    elif command -v rc-service >/dev/null 2>&1; then rc-service radiomoon-server stop 2>/dev/null || true; fi
}
svc_install() {
    if have_systemd; then
        install -m 644 "$HERE/radiomoon-server.service" /etc/systemd/system/radiomoon-server.service
        systemctl daemon-reload
        systemctl enable radiomoon-server >/dev/null 2>&1 || true
        systemctl restart radiomoon-server
        sleep 2
        systemctl --no-pager status radiomoon-server | head -5 || true
    elif command -v rc-service >/dev/null 2>&1; then
        install -m 755 "$HERE/radiomoon-server.openrc" /etc/init.d/radiomoon-server
        rc-update add radiomoon-server default >/dev/null 2>&1 || true
        rc-service radiomoon-server restart || rc-service radiomoon-server start
    else
        echo "No service manager found: start /opt/radiomoon/radiomoon-server by hand."
    fi
}
copy_program() {
    install -m 755 "$HERE/radiomoon-server" /opt/radiomoon/radiomoon-server
    [ -d "$HERE/lang" ] && cp -r "$HERE/lang" /opt/radiomoon/ || true
    # the clients (Windows, Linux, macOS) that the web page of the server offers for download
    if [ -d "$HERE/clients" ]; then
        rm -rf /opt/radiomoon/clients
        cp -r "$HERE/clients" /opt/radiomoon/clients
    fi
}

do_upgrade() {
    [ -x /opt/radiomoon/radiomoon-server ] || { echo "RadioMoon server is not installed here: run ./install.sh to install it."; exit 1; }
    OLD=$(/opt/radiomoon/radiomoon-server --version 2>/dev/null || echo "unknown")
    NEW=$("$HERE/radiomoon-server" --version 2>/dev/null || echo "unknown")
    echo "== Upgrade: $OLD  ->  $NEW"
    svc_stop
    cp -f /opt/radiomoon/radiomoon-server /opt/radiomoon/radiomoon-server.old 2>/dev/null || true
    copy_program
    chown -R radiomoon:radiomoon /var/lib/radiomoon 2>/dev/null || true
    echo "== Database (the tables are updated; a backup is made first)"
    su -s /bin/sh radiomoon -c "RADIOMOON_DATA=/var/lib/radiomoon /opt/radiomoon/radiomoon-server --setup" || {
        echo "The new program could not open the database. The old program is kept as /opt/radiomoon/radiomoon-server.old"
        exit 1
    }
    svc_install
    echo "Upgrade finished. Settings, database, songs and the web password were kept."
}

do_uninstall() {
    if [ "$PURGE" = 1 ] && [ "$YES" != 1 ]; then
        echo "This removes the program, the service, the RadioMoon database, the settings and the songs uploaded to the server."
        printf "Type yes to continue: "
        read ans
        [ "$ans" = "yes" ] || { echo "Nothing was removed."; exit 1; }
    fi
    echo "== Uninstall"
    svc_stop
    if have_systemd; then
        systemctl disable radiomoon-server >/dev/null 2>&1 || true
        rm -f /etc/systemd/system/radiomoon-server.service
        systemctl daemon-reload
    elif command -v rc-update >/dev/null 2>&1; then
        rc-update del radiomoon-server default >/dev/null 2>&1 || true
        rm -f /etc/init.d/radiomoon-server
    fi
    rm -rf /opt/radiomoon /etc/radiomoon /var/log/radiomoon-server.log
    if [ "$PURGE" = 1 ]; then
        SQL="DROP DATABASE IF EXISTS radiomoon; DROP USER IF EXISTS 'radiomoon'@'127.0.0.1'; DROP USER IF EXISTS 'radiomoon'@'localhost'; DROP USER IF EXISTS 'radiomoon'@'%'; FLUSH PRIVILEGES;"
        mariadb -e "$SQL" 2>/dev/null || mysql -e "$SQL" 2>/dev/null || echo "The database could not be removed (is MariaDB running?)."
        for d in /etc/my.cnf.d /etc/mysql/mariadb.conf.d /etc/mysql/conf.d; do rm -f "$d/99-radiomoon.cnf"; done
        rm -rf /var/lib/radiomoon
        if command -v userdel >/dev/null 2>&1; then userdel radiomoon 2>/dev/null || true; else deluser radiomoon 2>/dev/null || true; fi
        echo "Everything was removed. MariaDB itself stays installed (remove it with your package manager if you do not need it)."
    else
        echo "The program and the service were removed. Kept: the data folder /var/lib/radiomoon (settings, web password, uploaded songs) and the database 'radiomoon'."
        echo "To remove those too:  ./install.sh uninstall --purge"
    fi
}

case "$MODE" in
    upgrade) do_upgrade; exit 0 ;;
    uninstall|remove) do_uninstall; exit 0 ;;
    install|--purge|-y|"") ;;
    -h|--help|help) sed -n 2,8p "$0"; exit 0 ;;
    *) echo "Unknown option $MODE (install, upgrade, uninstall)"; exit 2 ;;
esac
if [ -x /opt/radiomoon/radiomoon-server ]; then
    echo "RadioMoon server is already installed: upgrading."
    do_upgrade
    exit 0
fi

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
    rc-update add mariadb default || true
    # it is started once, below, after its settings are written
elif command -v apt-get >/dev/null 2>&1; then
    DEBIAN_FRONTEND=noninteractive apt-get install -y mariadb-server libasound2
    systemctl enable --now mariadb || systemctl enable --now mysql || true
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y mariadb-server alsa-lib
    systemctl enable --now mariadb || true
fi
# Only this computer reaches the database. For the RadioMoon client on other computers run:  RADIOMOON_REMOTE_DB=1 ./install.sh
BIND=127.0.0.1
[ "$RADIOMOON_REMOTE_DB" = 1 ] && BIND=0.0.0.0
for d in /etc/my.cnf.d /etc/mysql/mariadb.conf.d /etc/mysql/conf.d; do
    if [ -d "$d" ]; then
        printf '[mysqld]\nbind-address=%s\nport=3306\n' "$BIND" > "$d/99-radiomoon.cnf"
        break
    fi
done
T=""; command -v timeout >/dev/null 2>&1 && T="timeout 90"
if command -v rc-service >/dev/null 2>&1 && [ ! -d /run/systemd/system ]; then
    $T rc-service mariadb restart || { echo "MariaDB did not restart cleanly: forcing it"; pkill -9 mariadbd 2>/dev/null; pkill -9 mysqld_safe 2>/dev/null; rc-service mariadb zap 2>/dev/null; rc-service mariadb start || true; }
else
    $T systemctl restart mariadb 2>/dev/null || $T systemctl restart mysql 2>/dev/null || true
fi
wait_db || echo "MariaDB does not answer yet; continuing."

echo "== Program"
id radiomoon >/dev/null 2>&1 || {
    if command -v useradd >/dev/null 2>&1; then useradd --system --home-dir /var/lib/radiomoon --shell /usr/sbin/nologin radiomoon
    else adduser -S -h /var/lib/radiomoon -s /sbin/nologin radiomoon; fi
}
getent group audio >/dev/null 2>&1 && (usermod -aG audio radiomoon 2>/dev/null || addgroup radiomoon audio 2>/dev/null || true)
mkdir -p /opt/radiomoon /var/lib/radiomoon /etc/radiomoon
copy_program
[ -f /etc/radiomoon/server.env ] || install -m 644 "$HERE/server.env" /etc/radiomoon/server.env
chown -R radiomoon:radiomoon /var/lib/radiomoon

echo "== Database for RadioMoon (user radiomoon, only from this computer)"
SQL="CREATE DATABASE IF NOT EXISTS radiomoon CHARACTER SET utf8mb4; CREATE USER IF NOT EXISTS 'radiomoon'@'127.0.0.1' IDENTIFIED BY 'radiomoon'; CREATE USER IF NOT EXISTS 'radiomoon'@'localhost' IDENTIFIED BY 'radiomoon'; GRANT ALL ON radiomoon.* TO 'radiomoon'@'127.0.0.1'; GRANT ALL ON radiomoon.* TO 'radiomoon'@'localhost'; FLUSH PRIVILEGES;"
if [ "$RADIOMOON_REMOTE_DB" = 1 ]; then
    SQL="$SQL CREATE USER IF NOT EXISTS 'radiomoon'@'%' IDENTIFIED BY 'radiomoon'; GRANT ALL ON radiomoon.* TO 'radiomoon'@'%'; FLUSH PRIVILEGES;"
    echo "The database accepts connections from the network: change the password of the database user 'radiomoon' and close port 3306 to everyone except your studio computers."
fi
mariadb -e "$SQL" 2>/dev/null || mysql -e "$SQL" || echo "Could not create the database automatically: create it by hand and run: /opt/radiomoon/radiomoon-server --setup --db-user ... --db-password ..."
su -s /bin/sh radiomoon -c "RADIOMOON_DATA=/var/lib/radiomoon /opt/radiomoon/radiomoon-server --setup --db-host 127.0.0.1 --db-port 3306 --db-user radiomoon --db-password radiomoon --db-name radiomoon" || echo "The server could not connect to the database (see above)."

svc_install
echo
echo "Password of the web studio:  grep password /var/lib/radiomoon/webserver.json"
echo "Open http://<this computer>:8080/ in a browser (user: admin)."
