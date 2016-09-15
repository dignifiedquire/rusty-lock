'use strict'

const fs = require('fs')
const lock = require('./')

const lockfile = './repo.lock'

console.log(lock.lock(lockfile))
setTimeout(()=> {
  console.log(lock.lock(lockfile))
}, 5000)
//console.log('unlocking')
//console.log(lock.unlock(lockfile))
