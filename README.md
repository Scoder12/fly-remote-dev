# fly-remote-dev

```sh
fly apps create --machines --name remote-dev
p | fly secrets import
fly vol create -s 2 home -r lax
fly ips allocate-v4 --shared
fly m list --json | jq -r '.[].id' | xargs fly m destroy --force
fly m run . --region lax --volume home:/home -s shared-cpu-8x
```
