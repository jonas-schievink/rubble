#!/usr/bin/env bash

set -o errexit

echo "TRAVIS_PULL_REQUEST=$TRAVIS_PULL_REQUEST"
echo "TRAVIS_BRANCH=$TRAVIS_BRANCH"

if [ "$TRAVIS_PULL_REQUEST" != "false" ]; then
    exit
fi

if [ "$TRAVIS_BRANCH" != "master" ]; then
    exit
fi

echo "Uploading documentation to GitHub Pages..."

echo "<meta http-equiv=refresh content=0;url=rubble/index.html>" > target/doc/index.html
sudo pip install ghp-import
ghp-import -n target/doc
git push -qf https://${GH_TOKEN}@github.com/jonas-schievink/rubble.git gh-pages
