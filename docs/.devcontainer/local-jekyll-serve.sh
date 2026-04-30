#!/bin/bash

SED_EXT=-E
LOCAL_CONFIG=_local_config.yml

cat _config.yml | sed ${SED_EXT} 's/^remote_theme:.+$/theme: just-the-docs/g' |
    sed ${SED_EXT} 's/^.+- jekyll-remote-theme.*$//g' > ${LOCAL_CONFIG}

bundle exec jekyll serve \
    --baseurl='' \
    --livereload \
    --force_polling \
    --config ${LOCAL_CONFIG}