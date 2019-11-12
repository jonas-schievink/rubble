#!/usr/bin/env bash

set -o errexit

if [ "$TRAVIS_PULL_REQUEST" == "false" ]; then
    exit
fi

if [ "$TRAVIS_BRANCH" != "master" ]; then
    exit
fi

echo "<meta http-equiv=refresh content=0;url=rubble/index.html>" > target/doc/index.html
sudo pip install ghp-import
ghp-import -n target/doc
git push -qf https://${GH_TOKEN}@github.com/jonas-schievink/rubble.git gh-pages
