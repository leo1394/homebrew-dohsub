class Dohsub < Formula
  desc "Local QX subscription server with private DNS-over-HTTPS node updates"
  homepage "https://github.com/leo1394/homebrew-dohsub"
  version "0.2.3"
  license all_of: ["MIT", "GPL-3.0-or-later"]

  head do
    url "https://github.com/leo1394/homebrew-dohsub.git", branch: "master"
    depends_on "go" => :build
    depends_on "rust" => :build
  end

  uses_from_macos "curl"

  on_macos do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.3/dohsub-aarch64-apple-darwin", using: :nounzip
      sha256 "181573a5e51df36c5d5d112c938c81ce7050cc232cf845c7451ef7f4723e81da"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.3/dohsub-x86_64-apple-darwin", using: :nounzip
      sha256 "0e6be8568a909bbb59d5aa875b9d073545177604005ecf0cefc808d11e3bbcbc"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.3/dohsub-aarch64-unknown-linux-gnu", using: :nounzip
      sha256 "b5c3ce8c6ec3c812ce876bf938a11628fb684d55498e5480d3f80038f0971fad"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.2.3/dohsub-x86_64-unknown-linux-gnu", using: :nounzip
      sha256 "134a0f42fc2d95856752b7d33d238815f6f823d17a4cfdb72d6469d067807443"
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
