'use strict'

const fs = require('fs')
const lock = require('./')

const lockfile = './repo.lock'

console.log(lock.lock(lockfile))
console.log('got lock')
