import sys
import os

def patch_file(path, old, new, desc):
    if not os.path.exists(path):
        print(f"⚠️ File not found: {path}")
        return False
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()

    content_norm = content.replace('\r\n', '\n')
    old_norm = old.replace('\r\n', '\n')
    new_norm = new.replace('\r\n', '\n')

    if old_norm in content_norm:
        content_norm = content_norm.replace(old_norm, new_norm)
        with open(path, 'w', encoding='utf-8') as f:
            f.write(content_norm)
        print(f"✅ {desc}")
        return True
    elif new_norm in content_norm:
        print(f"ℹ️ Already patched: {desc}")
        return True
    else:
        print(f"⚠️ Target block not found for: {desc}")
        return False

def patch_bitcrack(base_dir):
    # 1. Patch KeySearchDevice.h to add virtual destructor
    h_path = os.path.join(base_dir, "KeyFinderLib", "KeySearchDevice.h")
    patch_file(
        h_path,
        "class KeySearchDevice {\n\npublic:",
        "class KeySearchDevice {\n\npublic:\n    virtual ~KeySearchDevice() {}\n",
        "KeySearchDevice.h virtual destructor"
    )

    # 2. Patch CudaKeySearchDevice.cpp to make init() re-entrant with buffer cleanup & iteration reset
    cpp_path = os.path.join(base_dir, "CudaKeySearchDevice", "CudaKeySearchDevice.cpp")
    old_cuda_init = """    _stride = stride;

    cudaCall(cudaSetDevice(_device));

    // Block on kernel calls
    cudaCall(cudaSetDeviceFlags(cudaDeviceScheduleBlockingSync));"""

    new_cuda_init = """    _stride = stride;

    _iterations = 0;

    cudaCall(cudaSetDevice(_device));

    // Block on kernel calls (safe re-init for daemon mode)
    cudaError_t flagErr = cudaSetDeviceFlags(cudaDeviceScheduleBlockingSync);
    if(flagErr != cudaSuccess && flagErr != cudaErrorSetOnActiveProcess) {
        cudaCall(flagErr);
    }

    // Clean up previous run buffers to prevent memory leaks in daemon mode
    cleanupChainBuf();
    _deviceKeys.clearPublicKeys();
    _deviceKeys.clearPrivateKeys();
    _resultList.cleanup();"""

    patch_file(cpp_path, old_cuda_init, new_cuda_init, "CudaKeySearchDevice.cpp re-entrant buffer cleanup & iteration reset")

    # 3. Patch KeyFinder/main.cpp
    main_path = os.path.join(base_dir, "KeyFinder", "main.cpp")
    if os.path.exists(main_path):
        with open(main_path, 'r', encoding='utf-8') as f:
            content = f.read()

        # #include <sstream>
        if '#include <sstream>' not in content:
            content = '#include <sstream>\n' + content

        # RunConfig bool daemon = false;
        if 'bool daemon = false;' not in content:
            content = content.replace('bool follow = false;', 'bool follow = false;\n    bool daemon = false;')

        # resultCallback FOUND output
        if 'FOUND ' not in content:
            old_cb = 'void resultCallback(KeySearchResult info)\n{'
            new_cb = 'void resultCallback(KeySearchResult info)\n{\n\tstd::cout << "FOUND " << info.privateKey.toString(16) << " " << info.address << std::endl;\n\tstd::cout.flush();'
            content = content.replace(old_cb, new_cb)

        # CmdParse --daemon
        if '--daemon' not in content:
            content = content.replace(
                'parser.add("-u", "--uncompressed", false);',
                'parser.add("-u", "--uncompressed", false);\n\tparser.add("", "--daemon", false);'
            )
            content = content.replace(
                'if(optArg.equals("-t", "--threads")) {',
                'if(optArg.equals("", "--daemon")) {\n\t\t\t\t_config.daemon = true;\n\t\t\t} else if(optArg.equals("-t", "--threads")) {'
            )

        # Daemon execution loop in run() - NOTE: NEVER delete d inside the while loop!
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
                    } catch (const KeySearchException &e) {
                        std::cout << "ERROR " << e.msg << std::endl;
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

        content_norm = content.replace('\r\n', '\n')
        old_run_norm = old_run.replace('\r\n', '\n')
        new_run_norm = new_run.replace('\r\n', '\n')

        if old_run_norm in content_norm:
            content_norm = content_norm.replace(old_run_norm, new_run_norm)
            print("✅ KeyFinder/main.cpp patched with Persistent Daemon Mode!")
        elif new_run_norm in content_norm:
            print("ℹ️ KeyFinder/main.cpp already patched!")
        else:
            import re
            pattern = r"// Get device context[\s\S]*?delete d;"
            if re.search(pattern, content_norm):
                content_norm = re.sub(pattern, new_run_norm, content_norm, count=1)
                print("✅ KeyFinder/main.cpp patched using regex fallback!")
            else:
                print("❌ Failed to match device context block in KeyFinder/main.cpp")
                sys.exit(1)

        with open(main_path, 'w', encoding='utf-8') as f:
            f.write(content_norm)

if __name__ == '__main__':
    target = sys.argv[1] if len(sys.argv) > 1 else "."
    if os.path.isfile(target):
        base = os.path.dirname(os.path.dirname(os.path.abspath(target)))
    else:
        base = os.path.abspath(target)
    print(f"⚡ Applying cuBitCrack Daemon patches in: {base}")
    patch_bitcrack(base)
