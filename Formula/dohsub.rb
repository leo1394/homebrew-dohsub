class Dohsub < Formula
  desc "Local QX subscription server with private DNS-over-HTTPS node updates"
  homepage "https://github.com/leo1394/homebrew-dohsub"
  version "0.2.1"
  license all_of: ["MIT", "GPL-3.0-or-later"]

  head do
    url "https://github.com/leo1394/homebrew-dohsub.git", branch: "master"
    depends_on "go" => :build
    depends_on "rust" => :build
  end

  uses_from_macos "curl"

  on_macos do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.1/dohsub-aarch64-apple-darwin", using: :nounzip
      sha256 "b21b0216f5b3c7e5577655d7c1634972f25ef440286fc71dcff9cf3afe88d4f3"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.1/dohsub-x86_64-apple-darwin", using: :nounzip
      sha256 "bd8eb77f08168ee812e17eb9c77c4eb7178d04a566236c9a025fa1251d451207"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.1/dohsub-aarch64-unknown-linux-gnu", using: :nounzip
      sha256 "655c14e0339392e5590cc1333bdfaaf3f590f3934d8222f9dc1dd5131dec9cfb"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.1/dohsub-x86_64-unknown-linux-gnu", using: :nounzip
      sha256 "f288b510255e4473b55eccbb49c5bd3906521ae3d9de2c5cf706cb7ec6197440"
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
