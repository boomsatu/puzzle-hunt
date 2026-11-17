import sys
import os

def patch_main_cpp(file_path):
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()

    # 1. Add #include <sstream>
    if '#include <sstream>' not in content:
        content = '#include <sstream>\n' + content

    # 2. Add bool daemon = false; to RunConfig
    if 'bool daemon = false;' not in content:
        content = content.replace('bool follow = false;', 'bool follow = false;\n    bool daemon = false;')

    # 3. Add immediate FOUND output to resultCallback
    if 'FOUND ' not in content:
        old_cb = 'void resultCallback(KeySearchResult info)\n{'
        new_cb = 'void resultCallback(KeySearchResult info)\n{\n\tstd::cout << "FOUND " << info.privateKey.toString(16) << " " << info.address << std::endl;\n\tstd::cout.flush();'
        content = content.replace(old_cb, new_cb)

    # 4. Add --daemon to CmdParse
    if '--daemon' not in content:
        content = content.replace(
            'parser.add("-u", "--uncompressed", false);',
            'parser.add("-u", "--uncompressed", false);\n\tparser.add("", "--daemon", false);'
        )
        content = content.replace(
            'if(optArg.equals("-t", "--threads")) {',
            'if(optArg.equals("", "--daemon")) {\n\t\t\t\t_config.daemon = true;\n\t\t\t} else if(optArg.equals("-t", "--threads")) {'
        )

    # 5. Add interactive daemon loop to run()
    old_run = """        // Get device context
        KeySearchDevice *d = getDeviceContext(_devices[_config.device], _config.blocks, _config.threads, _config.pointsPerThread);

        KeyFinder f(_config.nextKey, _config.endKey, _config.compression, d, _config.stride);

        f.setResultCallback(resultCallback);
        f.setStatusInterval(_config.statusInterval);
        f.setStatusCallback(statusCallback);

        f.init();

        if(!_config.targetsFile.empty()) {
            f.setTargets(_config.targetsFile);
        } else {
            f.setTargets(_config.targets);
        }

        f.run();

        delete d;"""

    new_run = """        // Get device context
        KeySearchDevice *d = getDeviceContext(_devices[_config.device], _config.blocks, _config.threads, _config.pointsPerThread);

        if (_config.daemon) {
            std::cout << "DAEMON_READY" << std::endl;
            std::cout.flush();
            std::string line;
            while (std::getline(std::cin, line)) {
                util::removeNewline(line);
                line = util::trim(line);
                if (line.empty()) continue;
                std::stringstream ss(line);
                std::string cmd;
                ss >> cmd;
                if (cmd == "RANGE" || cmd == "JUMP") {
                    std::string s_start, s_end;
                    ss >> s_start >> s_end;
                    try {
                        secp256k1::uint256 startKey;
                        secp256k1::uint256 endKey;
                        parseKeyspace(s_start + ":" + s_end, startKey, endKey);

                        if (d != NULL) {
                            delete d;
                        }
                        d = getDeviceContext(_devices[_config.device], _config.blocks, _config.threads, _config.pointsPerThread);

                        KeyFinder f(startKey, endKey, _config.compression, d, _config.stride);
                        f.setResultCallback(resultCallback);
                        f.setStatusInterval(0);
                        f.init();
                        if(!_config.targetsFile.empty()) {
                            f.setTargets(_config.targetsFile);
                        } else {
                            f.setTargets(_config.targets);
                        }
                        uint64_t t0 = util::getSystemTime();
                        f.run();
                        uint64_t t1 = util::getSystemTime();
                        double elapsed = (t1 - t0) / 1000.0;
                        uint64_t count = (endKey - startKey).toUint64();
                        std::cout << "DONE " << count << " " << util::format("%.3f", elapsed) << std::endl;
                        std::cout.flush();
                    } catch (const std::exception &e) {
                        std::cout << "ERROR " << e.what() << std::endl;
                        std::cout.flush();
                    } catch (const std::string &err) {
                        std::cout << "ERROR " << err << std::endl;
                        std::cout.flush();
                    } catch (...) {
                        std::cout << "ERROR unknown" << std::endl;
                        std::cout.flush();
                    }
                } else if (cmd == "QUIT" || cmd == "EXIT") {
                    break;
                }
            }
        } else {
            KeyFinder f(_config.nextKey, _config.endKey, _config.compression, d, _config.stride);
            f.setResultCallback(resultCallback);
            f.setStatusInterval(_config.statusInterval);
            f.setStatusCallback(statusCallback);
            f.init();
            if(!_config.targetsFile.empty()) {
                f.setTargets(_config.targetsFile);
            } else {
                f.setTargets(_config.targets);
            }
            f.run();
        }

        if (d != NULL) {
            delete d;
        }"""

    # Normalize line endings before replacing
    content_norm = content.replace('\r\n', '\n')
    old_run_norm = old_run.replace('\r\n', '\n')
    new_run_norm = new_run.replace('\r\n', '\n')

    if old_run_norm in content_norm:
        content_norm = content_norm.replace(old_run_norm, new_run_norm)
        print("✅ Successfully patched KeyFinder/main.cpp with Interactive Daemon Mode!")
    else:
        print("⚠️ Warning: old_run block not matched exactly, trying fallback regex...")
        import re
        pattern = r"// Get device context[\s\S]*?delete d;"
        if re.search(pattern, content_norm):
            content_norm = re.sub(pattern, new_run_norm, content_norm, count=1)
            print("✅ Successfully patched KeyFinder/main.cpp using fallback pattern!")
        else:
            print("❌ Failed to match device context block in KeyFinder/main.cpp")
            sys.exit(1)

    with open(file_path, 'w', encoding='utf-8') as f:
        f.write(content_norm)

if __name__ == '__main__':
    if len(sys.argv) > 1:
        patch_main_cpp(sys.argv[1])
    else:
        print("Usage: python patch_cubitcrack.py <path_to_KeyFinder/main.cpp>")
