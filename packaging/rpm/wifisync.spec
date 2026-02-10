%global crate wifisync
%global debug_package %{nil}

Name:           wifisync
Version:        0.1.0
Release:        1%{?dist}
Summary:        Wifi credential synchronization tool

License:        MIT OR Apache-2.0
URL:            https://github.com/bkero/wifisync
Source0:        %{name}-%{version}.tar.gz

# Note: rust/cargo are provided by rustup in Docker build environment
BuildRequires:  gcc
BuildRequires:  dbus-devel
BuildRequires:  systemd-rpm-macros

Requires:       NetworkManager
Requires:       dbus

%description
Wifisync is a tool for synchronizing wifi credentials across devices.
It securely stores wifi passwords in an encrypted database and provides
them to NetworkManager on-demand via a Secret Agent daemon.

Features:
- Extract wifi credentials from NetworkManager
- Encrypted local storage with ChaCha20-Poly1305
- Filter out enterprise and open networks
- Export/import credentials for sharing
- Secret Agent daemon for on-demand password delivery

%prep
%autosetup -n %{name}-%{version}

%build
%if ! 0%{?skip_build}
cargo build --release --locked
%endif

%install
# Install binary
install -Dm755 target/release/wifisync %{buildroot}%{_bindir}/wifisync

# Install systemd user service
install -Dm644 packaging/systemd/wifisync-agent.service %{buildroot}%{_userunitdir}/wifisync-agent.service


%post
%systemd_user_post wifisync-agent.service

%preun
%systemd_user_preun wifisync-agent.service

%postun
%systemd_user_postun_with_restart wifisync-agent.service

%files
%license LICENSE-MIT LICENSE-APACHE
%{_bindir}/wifisync
%{_userunitdir}/wifisync-agent.service

%changelog
* Mon Jan 27 2025 Wifisync Developers <dev@wifisync.example.com> - 0.1.0-1
- Initial package release
- Core credential extraction and storage
- NetworkManager adapter with Secret Agent support
- Encrypted storage with ChaCha20-Poly1305
