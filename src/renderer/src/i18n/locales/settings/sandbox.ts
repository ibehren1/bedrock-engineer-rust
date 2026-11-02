export const sandboxSettings = {
  en: {
    sandboxSettings: {
      environment: {
        title: 'Docker',
        description:
          'The sandbox runs each chat in its own Docker container. Docker must be installed and running; Compose is optional and only needed for multi-service sandboxes.'
      },
      image: {
        title: 'Default container image',
        description:
          'The operating system new sandboxes are built on. The agent can still ask for a different one when it creates a sandbox.',
        label: 'Operating system',
        hint: 'Used for new sandboxes. A chat that already has a sandbox keeps its OS until the sandbox is removed.',
        debian: 'Quickest to download and to install packages on. Uses apt-get.',
        ubuntu: 'Uses apt-get.',
        rhel: 'Uses dnf.',
        ubi: 'Real RHEL, limited to Red Hat’s freely available packages. Uses dnf.'
      },
      restrictions: {
        title: 'Sandbox restrictions',
        description:
          'Optional limits for new sandbox containers. Both are off by default because they get in the way of ordinary work in a sandbox. They apply to sandboxes created after you turn them on; remove and recreate a sandbox to apply them to it.',
        turnsOff: 'What this turns off:',
        lockDown: {
          label: 'Lock down sandbox containers',
          protects:
            'Protects against: a command the agent was steered into running gaining more power inside the container (for example through a setuid program), sending crafted network packets, or starting endless processes that take down Docker. Commands still run as root, and installing packages with apt or dnf still works.',
          turnsOff: [
            'sudo, su and any other setuid program (they can no longer gain privileges)',
            'ping, traceroute and other tools that need raw network sockets',
            'Creating device files (mknod) and chroot',
            'Writing to the kernel audit log',
            'Running more than 1,024 processes at once (large parallel builds can hit this)',
            'Any other Linux capability not needed to install packages, such as changing the network setup or mounting filesystems'
          ]
        },
        network: {
          label: 'Start sandboxes with no network access',
          protects:
            "Protects against: a command in the sandbox reaching the internet, services on this computer (through Docker's host address), other devices on your network, or on a cloud machine the instance metadata service that hands out its temporary credentials, and sending out what it finds.",
          turnsOff: [
            'All internet access from the sandbox',
            'Installing or updating packages (apt, dnf, pip, npm and so on)',
            'git clone, pulling images inside the container and downloading files',
            'Reaching services on this computer or your local network',
            'Published ports: you can no longer open what the agent runs in a browser',
            'Services in a multi-service stack talking to each other'
          ]
        }
      },
      limits: {
        title: 'Container limits',
        memory: 'Memory limit',
        cpu: 'CPU limit',
        timeout: 'Command timeout'
      },
      toolSummary:
        'Current settings: {{os}}, {{memory}} memory, {{cpu}}, {{timeout}} command timeout.',
      openSettings: 'Open sandbox settings'
    }
  },
  ja: {
    sandboxSettings: {
      environment: {
        title: 'Docker',
        description:
          'サンドボックスはチャットごとに専用の Docker コンテナで動作します。Docker のインストールと起動が必要です。Compose は任意で、複数サービスのサンドボックスにのみ必要です。'
      },
      image: {
        title: '既定のコンテナイメージ',
        description:
          '新しいサンドボックスで使う OS です。エージェントはサンドボックス作成時に別の OS を指定することもできます。',
        label: 'オペレーティングシステム',
        hint: '新しいサンドボックスに適用されます。既にサンドボックスがあるチャットは、削除するまで現在の OS のままです。',
        debian: 'ダウンロードとパッケージのインストールが最も速い OS です。apt-get を使います。',
        ubuntu: 'apt-get を使います。',
        rhel: 'dnf を使います。',
        ubi: '本物の RHEL ですが、Red Hat が無償提供するパッケージのみ利用できます。dnf を使います。'
      },
      restrictions: {
        title: 'サンドボックスの制限',
        description:
          '新しいサンドボックスコンテナーに任意でかける制限です。サンドボックスでの普段の作業の妨げになるため、どちらも既定ではオフです。オンにした後に作成するサンドボックスに適用されます。既存のサンドボックスに適用するには削除して作り直してください。',
        turnsOff: 'これによって使えなくなるもの:',
        lockDown: {
          label: 'サンドボックスコンテナーをロックダウンする',
          protects:
            '防ぐもの: エージェントが誘導されて実行したコマンドが、コンテナー内でより強い権限を得ること（setuid プログラム経由など）、細工したネットワークパケットを送ること、際限なくプロセスを起動して Docker を停止させること。コマンドは引き続き root で実行され、apt や dnf によるパッケージのインストールも使えます。',
          turnsOff: [
            'sudo、su などの setuid プログラム（権限を得られなくなります）',
            'ping、traceroute など raw ソケットが必要なツール',
            'デバイスファイルの作成（mknod）と chroot',
            'カーネル監査ログへの書き込み',
            '同時に 1,024 を超えるプロセスの実行（大規模な並列ビルドでは上限に達することがあります）',
            'ネットワーク設定の変更やファイルシステムのマウントなど、パッケージのインストールに不要なその他の Linux ケーパビリティ'
          ]
        },
        network: {
          label: 'ネットワークなしでサンドボックスを起動する',
          protects:
            '防ぐもの: サンドボックス内のコマンドがインターネット、このコンピューター上のサービス（Docker のホストアドレス経由）、ネットワーク上の他の機器、クラウド上のマシンでは一時的な認証情報を渡すインスタンスメタデータサービスにアクセスし、見つけた内容を外部へ送ること。',
          turnsOff: [
            'サンドボックスからのすべてのインターネットアクセス',
            'パッケージのインストールや更新（apt、dnf、pip、npm など）',
            'git clone、コンテナー内でのイメージの取得、ファイルのダウンロード',
            'このコンピューターやローカルネットワーク上のサービスへのアクセス',
            'ポートの公開（エージェントが起動したものをブラウザーで開けなくなります）',
            '複数サービス構成でのサービス同士の通信'
          ]
        }
      },
      limits: {
        title: 'コンテナの制限',
        memory: 'メモリ上限',
        cpu: 'CPU 上限',
        timeout: 'コマンドのタイムアウト'
      },
      toolSummary:
        '現在の設定: {{os}}、メモリ {{memory}}、{{cpu}}、コマンドのタイムアウト {{timeout}}。',
      openSettings: 'サンドボックス設定を開く'
    }
  }
}
