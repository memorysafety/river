# Hot Reloading

River does not support changing most settings while the server is running.
In order to change the settings of a running instance of River, it is necessary to
launch a new instance of River.

However, River does support "Hot Reloading" - the ability for a new instance of
River to take over the responsibilities of a currently executing server.

From a high level view, this process looks like:

1. The existing instance of River is running
2. A new instance of River is started, configured with "upgrade" enabled via the command line.
   The new instance does not yet begin execution, and is waiting for a hand-over of Listeners
   from the existing instance
3. A SIGQUIT signal is sent to the FIRST River instance, which causes it to stop accepting
   new connections, and to transfer all active listening Listener file descriptors to the
   SECOND River instance
4. The SECOND River instance begins listening to all Listeners, and operating normally
5. The FIRST River instance continues handling any currently active downstream connections,
   until either all connections have closed, or until a timeout period is reached. If
   the timeout is reached, all open connections are closed ungracefully.
6. At the end of the timeout period, the FIRST River instance exits.

In most cases, this allows seamless hand over from the OLD instance of RIVER to the NEW
instance of River, without any interruption of service. As long as no connections are
longer-lived than the timeout period, then this hand-over will not be observable from
downstream clients.

Once the SIGQUIT signal is sent, all new incoming connections will be handled by the
new instance of River. Existing connections will continue to be serviced by the old
instance until their connection has been closed.

There are a couple moving pieces that are necessary for this process to occur:

## pidfile

When River is configured to be daemonized, it will create a pidfile containing its
process ID at the configured location.

This file can be used to determine the process ID necessary for sending SIGQUIT to.

When the second instance has taken over, the pidfile of the original instance
will be replaced with the pidfile of the new instance.

In general, both instances of River should be configured with the same
pidfile path.

## upgrade socket

In order to facilitate the transfer of listening socket file descriptors from
one instance to another, a socket is used to transfer file descriptors.

This transfer begins when the SIGQUIT signal is sent to the first process.

Both instances of River MUST be configured with the same upgrade socket path.
