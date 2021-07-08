FROM debian:buster-slim
RUN apt-get update -q && apt-get install -qqy tor
COPY contrib/torrc /etc/tor/torrc
VOLUME [ "/var/lib/tor/onion_service" ]
# Set the permissions expected by the Tor daemon and start it
CMD chown -R root:root /var/lib/tor/onion_service && chmod 700 /var/lib/tor/onion_service \
    && tor