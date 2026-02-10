%global crate wifisync-server
%global debug_package %{nil}

Name:           wifisync-server
Version:        0.1.0
Release:        1%{?dist}
Summary:        Wifi credential sync server for Wifisync

License:        MIT OR Apache-2.0
URL:            https://github.com/bkero/wifisync
Source0:        wifisync-%{version}.tar.gz

# Note: rust/cargo are provided by rustup in Docker build environment
BuildRequires:  gcc
BuildRequires:  systemd-rpm-macros

Requires:       sqlite
Requires(pre):  shadow-utils

%description
Wifisync Server is the synchronization backend for Wifisync clients.
It provides a REST API for pushing and pulling encrypted wifi credentials
across multiple devices.

Features:
- REST API with JWT authentication
- SQLite database for easy self-hosting
- End-to-end encryption (server never sees plaintext passwords)
- Vector clock-based conflict detection
- Multi-device synchronization

%prep
%autosetup -n wifisync-%{version}

%build
%if ! 0%{?skip_build}
cargo build --release --locked -p wifisync-server
%endif

%install
# Install binary
install -Dm755 target/release/wifisync-server %{buildroot}%{_bindir}/wifisync-server

# Install systemd system service
install -Dm644 packaging/systemd/wifisync-server.service %{buildroot}%{_unitdir}/wifisync-server.service

# Create data directory
install -dm750 %{buildroot}%{_sharedstatedir}/wifisync-server

# Install sysusers.d config
install -Dm644 /dev/stdin %{buildroot}%{_sysusersdir}/wifisync-server.conf << 'EOF'
u wifisync-server - "Wifisync Server" /var/lib/wifisync-server /sbin/nologin
EOF

# Install tmpfiles.d config
install -Dm644 /dev/stdin %{buildroot}%{_tmpfilesdir}/wifisync-server.conf << 'EOF'
d /var/lib/wifisync-server 0750 wifisync-server wifisync-server -
EOF

%pre
%sysusers_create_package wifisync-server %{_sysusersdir}/wifisync-server.conf

%post
%systemd_post wifisync-server.service

%preun
%systemd_preun wifisync-server.service

%postun
%systemd_postun_with_restart wifisync-server.service

%files
%license LICENSE-MIT LICENSE-APACHE
%{_bindir}/wifisync-server
%{_unitdir}/wifisync-server.service
%{_sysusersdir}/wifisync-server.conf
%{_tmpfilesdir}/wifisync-server.conf
%dir %attr(750,wifisync-server,wifisync-server) %{_sharedstatedir}/wifisync-server

%changelog
* Fri Jan 31 2025 Wifisync Developers <dev@wifisync.example.com> - 0.1.0-1
- Initial server package release
- REST API with JWT authentication
- SQLite database storage
- End-to-end encrypted credential sync
- Vector clock conflict detection
