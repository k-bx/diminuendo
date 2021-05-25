# Setup

- Clone on your raspberry pi in `~/src/diminuendo`
- Build via `cargo build --release`
- Put `sysadmin/diminuendo.service` in `/etc/systemd/system`; do `sudo systemctl daemon-reload`; `sudo systemctl enable diminuendo`; `sudo systemctl start diminuendo`

Your data is written in `~/storage/diminuendo.sqlite`

```
pi@raspberrypi:~/storage $ sqlite3 diminuendo.sqlite
SQLite version 3.27.2 2019-02-25 16:06:06
Enter ".help" for usage hints.
sqlite> select datetime(ts, 'unixepoch'),length(events),events from events order by ts desc limit 10;
2021-05-25 07:19:05|9|  0       <       7
2021-05-25 07:19:00|4|  <"
2021-05-25 07:18:59|4|  7"
2021-05-25 07:18:59|4|  0)
2021-05-25 07:17:38|6|  3       8
2021-05-25 07:17:34|3|  5
2021-05-25 07:17:34|8|  3&      8&
2021-05-25 07:17:34|3|  :
2021-05-25 07:17:33|3|  2
2021-05-25 07:17:33|4|  5$
```

# TODO

- [x] write events to sqlite
- [ ] **p1** relaunch on start
- [ ] **p1** reconnect on piano turned on/off
- [ ] **p1** play recordings on mobile web
- [ ] **p2** auto-select USB device
- [ ] **p2** create/migrate database automatically
- [ ] **p2** choose db from current dir if not present elsewhere
