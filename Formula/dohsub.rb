class Dohsub < Formula
  desc "Local QX subscription server with private DNS-over-HTTPS node updates"
  homepage "https://github.com/leo1394/homebrew-dohsub"
  version "0.2.2"
  license all_of: ["MIT", "GPL-3.0-or-later"]

  head do
    url "https://github.com/leo1394/homebrew-dohsub.git", branch: "master"
    depends_on "go" => :build
    depends_on "rust" => :build
  end

  uses_from_macos "curl"

  on_macos do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.2/dohsub-aarch64-apple-darwin", using: :nounzip
      sha256 "502b707272b5097a1d81153155cbafc9c7fad38faaa1828f8b1b88edd7155270"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.2/dohsub-x86_64-apple-darwin", using: :nounzip
      sha256 "71fa2e2a3959e20afc6ced228a138bd451ee447641b25c97c91cffa34890f122"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.2/dohsub-aarch64-unknown-linux-gnu", using: :nounzip
      sha256 "531f40106e9da562ae6e3a399ebe46a5ecc7bd1230d2f2e6566333a61059b0da"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.2/dohsub-x86_64-unknown-linux-gnu", using: :nounzip
      sha256 "d18b770dcab1be67cf486cce9f2f79b3d80f7416bbdd4a54eb3f003c2080571a"
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
