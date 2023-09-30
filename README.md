# P3D Pool Proxy

## Table of Contents
- [Introduction](#introduction)
- [Features](#features)
- [Usage](#usage)
- [Configuration](#configuration)
- [Contributing](#contributing)

## Introduction

P3D Pool Proxy is a lightweight proxy application designed to facilitate communication between any unofficial 3dpass miner and a mining pool or node. This proxy simplifies the process of connecting your miner to a pool, ensuring efficient and reliable mining operations.

![P3D Pool Proxy](/screenshots/screenshot.png)

## Features

- **Proxy Service:** P3D Pool Proxy acts as an intermediary between your P3D miner and the mining pool or node, optimizing data transmission.

- **Configuration Options:** Customize various parameters to suit your specific mining requirements.

## Usage

To use P3D Pool Proxy, follow these steps:

1. Start the proxy using the binary created during installation.

    ```bash
    ./target/release/p3d-pool-proxy run --node-url 127.0.0.1:9933 --pool-url 0.0.0.0:3333 --pool-id [POOL_ID] --member-id [WALLET] --member-key [MEMBER_PRIVATE_KEY]
    ```

2. Connect your unofficial P3D miner to the proxy's address and port (e.g., `localhost:3333`) instead of directly to the mining pool.

3. The proxy exposes the following JSON-RPC methods:

    1. `get_meta`: This method does not require any parameters.
    2. `push_to_node`: This method receives a `hash` (string) and an `obj` (string) as parameters.
    3. `get_mining_params`: This method receives `pool_id` (string) as a parameter.
    4. `push_to_pool`: This method receives a `hash` (string) and an `obj` (string) as parameters.

    Use these JSON-RPC methods to interact with the proxy.

4. Monitor the proxy's logs to ensure smooth operation.

## Configuration

Here are some of the configurable options:

- `--node-url`: The URL of the mining pool to which the proxy should forward traffic.

- `--pool-url`: The IP address or hostname on which the proxy should listen.

- `--pool-id`: The pool id.

- `--member-id`: The member wallet.

- `--member-key`: The member private key.

## Contributing

We welcome contributions to P3D Pool Proxy. To contribute, follow these steps:

1. Fork the repository.

2. Create a new branch for your feature or bug fix:

    ```bash
    git checkout -b feature-name
    ```

3. Make your changes and commit them:

    ```bash
    git commit -m "Description of your changes"
    ```

4. Push your changes to your forked repository:

    ```bash
    git push origin feature-name
    ```

5. Create a pull request on the original repository, describing your changes and their purpose.

We appreciate your contributions!

Feel free to modify and expand this README to better suit your project's needs. Good luck with your P3D Pool Proxy project!

## Donations

If you want to support my work, please feel free to send your donations to:

P3D: d1CVfTXNxP73KXoBf7gbwNnBVF9hqtJJ1ZAxGEfgTdLboj8UV

XRP: r3vcfYF3aJwqRbDaUEm19Tk2BCQiwjBHBg

BTC: bc1qsgt7urjpkhcjcyengvszqkzzh6wunwegxe7wsh

ETH: 0x09045794c650a86885196157bc1891c8719267Bd
