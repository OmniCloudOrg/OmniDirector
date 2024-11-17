FROM mcr.microsoft.com/vscode/devcontainers/base:ubuntu-20.04

COPY ./data/target-installers/* /target-installers/

RUN /target-installers/*/*.sh

COPY . /cached-code/

WORKDIR /cached-code/

CMD [ "./run.sh" ] 
# example