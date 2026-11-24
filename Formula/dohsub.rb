class Dohsub < Formula
  desc "Local QX subscription server with private DNS-over-HTTPS node updates"
  homepage "https://github.com/leo1394/homebrew-dohsub"
  version "0.2.0"
  license all_of: ["MIT", "GPL-3.0-or-later"]

  head do
    url "https://github.com/leo1394/homebrew-dohsub.git", branch: "master"
    depends_on "go" => :build
    depends_on "rust" => :build
  end

  uses_from_macos "curl"

  on_macos do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.0/dohsub-aarch64-apple-darwin", using: :nounzip
      sha256 "5996d89af8c4f350882483768173f5701bd698ed09d1baf7f650f6f4bf5766e4"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.0/dohsub-x86_64-apple-darwin", using: :nounzip
      sha256 "bf199b303b493ff39ef641e479163436832407be1af21e5665cd9dfe5283b20f"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.0/dohsub-aarch64-unknown-linux-gnu", using: :nounzip
      sha256 "e404f639d662348f319e87643722eead444200a3411eba3e352c8e795cd6ff67"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.0/dohsub-x86_64-unknown-linux-gnu", using: :nounzip
      sha256 "394211359278ea4d4490b6dc1f6df7ba586ddb8ceb19088de212946180138b22"
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
