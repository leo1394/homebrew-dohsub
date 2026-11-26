class Dohsub < Formula
  desc "Local QX subscription server with private DNS-over-HTTPS node updates"
  homepage "https://github.com/leo1394/homebrew-dohsub"
  version "0.2.8"
  license all_of: ["MIT", "GPL-3.0-or-later"]

  head do
    url "https://github.com/leo1394/homebrew-dohsub.git", branch: "master"
    depends_on "go" => :build
    depends_on "rust" => :build
  end

  uses_from_macos "curl"

  on_macos do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.8/dohsub-aarch64-apple-darwin", using: :nounzip
      sha256 "7954322fce588e7f5d003ed94ff8cf0265ca36986e55a216cff9fee7f3777d2e"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.8/dohsub-x86_64-apple-darwin", using: :nounzip
      sha256 "83fc063c01080c9abe21d68afdb41cde55c7d09294222a9602adec0c69b60736"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.8/dohsub-aarch64-unknown-linux-gnu", using: :nounzip
      sha256 "c460e2a2f89e4b6327ec9836cb20a42391395696fb9ac412649f8cbeb948ec02"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.8/dohsub-x86_64-unknown-linux-gnu", using: :nounzip
      sha256 "192028de17b582f293e5bf1db637c61f7de1fa158d69a762efa03b5670070fd5"
    end
  end

  def install
    odie "dohsub requires macOS Monterey or later" if OS.mac? && MacOS.version < :monterey
    if build.head?
      system "cargo", "install", *std_cargo_args
    else
      bin.install Dir["dohsub-*"].first => "dohsub"
    end
    chmod 0755, bin/"dohsub"
    generate_completions_from_executable(bin/"dohsub", "completion")
    man1.mkpath
    (man1/"dohsub.1").write Utils.safe_popen_read(bin/"dohsub", "man")
  end

  test do
    output = shell_output("#{bin}/dohsub --version")
    assert_match "GNU General Public License", shell_output("#{bin}/dohsub relay-license")
    assert_equal 2, output.lines.count
    assert_match "https://github.com/leo1394/homebrew-dohsub", output
    ENV["DOHSUB_HOME"] = testpath/"private-state"
    system bin/"dohsub", "config", "set", "--port", "55355"
    assert_match '"port": 55355', shell_output("#{bin}/dohsub config --list")
    assert_path_exists man1/"dohsub.1"
    assert_path_exists bash_completion/"dohsub"
    assert_path_exists zsh_completion/"_dohsub"
    assert_path_exists fish_completion/"dohsub.fish"
  end
end
