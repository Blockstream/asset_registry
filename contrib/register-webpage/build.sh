#!/bin/bash

cat register.pug | pug | html-inline > register.html
