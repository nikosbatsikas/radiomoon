# RadioMoon server in a container (Alpine). Built by the release workflow from the Alpine server programs:
#   context: ctx/amd64/radiomoon-server, ctx/arm64/radiomoon-server, lang/, clients/, entrypoint.sh
FROM alpine:3.20
RUN apk add --no-cache alsa-lib libgcc libstdc++ ca-certificates tzdata tini tar
ARG TARGETARCH
COPY ctx/${TARGETARCH}/radiomoon-server /opt/radiomoon/radiomoon-server
COPY lang /opt/radiomoon/lang
COPY clients /opt/radiomoon/clients
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /opt/radiomoon/radiomoon-server /entrypoint.sh \
 && addgroup -S radiomoon && adduser -S -G radiomoon -h /data radiomoon \
 && mkdir -p /data && chown radiomoon:radiomoon /data
ENV RADIOMOON_DOCKER=1 \
    RADIOMOON_DATA=/data \
    RADIOMOON_BIND=0.0.0.0:8080
VOLUME /data
EXPOSE 8080
USER radiomoon
ENTRYPOINT ["/sbin/tini", "--", "/entrypoint.sh"]
