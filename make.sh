#!/bin/bash

verbose() { echo "$0 [-sb] [-c <client-port>,<bootstrap-addr-CIDR>]" 1>&2; exit 1; }

cargo_build() {
    cd $1 || exit 1
    cargo build
    cd ../
}

cargo_run_bootstrap() {
    RUST_LOG=info cargo r
}

cargo_run_client() {
    cd auctions_cli || exit 1
    cargo r $1 $2
    cd ../
    exit 0

}

while getopts "c :s :b" opt; do
set -f; IFS=','
args=($2)
    case "${opt}" in
        c)
            cargo_run_client ${args[0]} ${args[1]}
            ;;
        s)
            echo "running bootstrap (verbose on)"
            cargo_run_bootstrap
            ;;
        b)
            echo "source build"
            cargo_build src
            echo "client cli build"
            cargo_build auctions_cli
            ;;
        *)
            verbose
            ;;
    esac
done

verbose