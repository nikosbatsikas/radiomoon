# Users and Two-step Login

**Control Panel > Settings > Users & security**.

![Users](images/screens/cp-users.png)

Turn on **Enable user permissions** (add at least one administrator first). Each user has a name, a password, an automatic **logout time** (0 = off), and either **Administrator** (all) or the list of permissions to tick (libraries, schedules, settings, utilities...). The permissions apply in the program window, in the client and in the web studio.

Besides these users there is the **web administrator** `admin` of the server, whose password is in `webserver.json`.

## Two-step login (MFA)

Each user of a server can have a second step: the code of an **authenticator app** (Google Authenticator, Microsoft Authenticator, Authy, 2FAS ...).

1. Edit the user, press **Turn on two-step login**.
2. Scan the **QR code** with the app, or type the key. (The `otpauth://` address is shown too.)
3. Type the 6 digit code that the app shows and press **Confirm**, then **OK**.

![Two-step login](images/screens/cp-user-mfa.png)

After that the user signs in with the password **followed by the code**:

- the **client** has a *Two-step code* field;
- the **web studio** has a code field in the sign-in page;
- other programs: the password field is `password123456`.

A checked sign-in stays valid for **12 hours** (the browser and the client send the same credentials with every request and a code is only valid for 30 seconds). After **5 wrong codes** the user is locked for 5 minutes. If the server is restarted everybody signs in again with a new code. **Turn off two-step login** in the same window. The `admin` of `webserver.json` has no two-step login.

For access from the internet use HTTPS as well: see [[Cloud Hosting Design]].
