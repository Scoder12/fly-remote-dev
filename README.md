# fly-remote-dev

```sh
fly apps create --machines --name remote-dev
p | fly secrets import
fly vol create -s 2 home -r lax
fly ips allocate-v4 --shared
fly m list --json | jq -r '.[].id' | xargs fly m destroy --force
fly m run . --region lax --volume home:/home -s shared-cpu-8x
```

Install extensions:

```sh
code-server --install-extension rust-lang.rust-analyzer; \
code-server --install-extension vscodevim.vim; \
code-server --install-extension eamodio.gitlens; \
code-server --install-extension PKief.material-icon-theme; \
code-server --install-extension bungcip.better-toml; \
code-server --install-extension usernamehw.errorlens; \
code-server --install-extension FelixIcaza.andromeda; \
code-server --install-extension esbenp.prettier-vscode
```
