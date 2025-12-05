    <script>
        // Global state
        var currentUser = null;
        var selectedImagePath = null;
        var selectedImageName = null;
        var carrierImageBase64 = null;
        var decryptedImageUrl = null;
        var API_BASE = window.location.origin + '/api';
        var myImages = [];
        var refreshInterval = null;

        // Auth Functions
        function switchAuthTab(tab) {
            document.querySelectorAll('.auth-section .nav-tab').forEach(function (t) {
                t.classList.remove('active');
            });
            document.querySelectorAll('.auth-section .tab-content').forEach(function (c) {
                c.classList.remove('active');
            });

            if (tab === 'signin') {
                document.querySelectorAll('.auth-section .nav-tab')[0].classList.add('active');
                document.getElementById('signin-tab').classList.add('active');
            } else {
                document.querySelectorAll('.auth-section .nav-tab')[1].classList.add('active');
                document.getElementById('signup-tab').classList.add('active');
            }
        }

        function signUp() {
            var clientId = document.getElementById('signup-clientid').value.trim();
            if (!clientId) {
                showMessage('auth-message', 'Please enter a client ID', 'error');
                return;
            }

            fetch(API_BASE + '/signup', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ client_id: clientId })
            })
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    if (data.success) {
                        currentUser = clientId;
                        localStorage.setItem('currentUser', currentUser);
                        showMainSection();
                        showMessage('auth-message', 'Sign up successful!', 'success');
                        startAutoRefresh();
                    } else {
                        showMessage('auth-message', data.error || 'Sign up failed', 'error');
                    }
                })
                .catch(function (error) {
                    showMessage('auth-message', 'Error: ' + error.message, 'error');
                });
        }

        function signIn() {
            var clientId = document.getElementById('signin-clientid').value.trim();
            if (!clientId) {
                showMessage('auth-message', 'Please enter a client ID', 'error');
                return;
            }

            fetch(API_BASE + '/signin', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ client_id: clientId })
            })
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    if (data.success) {
                        currentUser = clientId;
                        localStorage.setItem('currentUser', currentUser);
                        showMainSection();
                        if (data.notifications && data.notifications.length > 0) {
                            showMessage('auth-message', data.notifications.length + ' new notifications!', 'info');
                        }
                        startAutoRefresh();
                    } else {
                        showMessage('auth-message', data.error || 'Sign in failed', 'error');
                    }
                })
                .catch(function (error) {
                    showMessage('auth-message', 'Error: ' + error.message, 'error');
                });
        }

        function signOut() {
            if (!currentUser) return;

            stopAutoRefresh();

            fetch(API_BASE + '/signout', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ client_id: currentUser })
            })
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    currentUser = null;
                    localStorage.removeItem('currentUser');
                    showAuthSection();
                })
                .catch(function (error) {
                    console.error('Sign out error:', error);
                });
        }

        function showMainSection() {
            document.getElementById('auth-section').classList.remove('active');
            document.getElementById('main-section').classList.add('active');
            document.getElementById('current-user').textContent = currentUser;
            loadMyImages();
            loadPendingRequests();
        }

        function showAuthSection() {
            document.getElementById('auth-section').classList.add('active');
            document.getElementById('main-section').classList.remove('active');
        }

        // Auto-refresh every 5 seconds
        function startAutoRefresh() {
            if (refreshInterval) return;
            refreshInterval = setInterval(function() {
                loadMyImages();
                loadPendingRequests();
                loadRequestedImages();
            }, 5000);
        }

        function stopAutoRefresh() {
            if (refreshInterval) {
                clearInterval(refreshInterval);
                refreshInterval = null;
            }
        }

        // Tab switching
        function switchTab(tab) {
            document.querySelectorAll('.main-section .nav-tab').forEach(function (t) {
                t.classList.remove('active');
            });
            document.querySelectorAll('.main-section .tab-content').forEach(function (c) {
                c.classList.remove('active');
            });

            var tabs = ['encrypt', 'dos', 'request-image', 'requested', 'shared', 'access', 'pending'];
            var index = tabs.indexOf(tab);
            if (index >= 0) {
                document.querySelectorAll('.main-section .nav-tab')[index].classList.add('active');
                document.getElementById(tab + '-tab').classList.add('active');
            }

            // Auto-load data when switching to certain tabs
            if (tab === 'encrypt') {
                loadMyImagesForEncrypt();
            } else if (tab === 'dos') {
                refreshOnlinePeers();
            } else if (tab === 'shared') {
                loadMyImages();
            } else if (tab === 'requested') {
                loadRequestedImages();
            } else if (tab === 'pending') {
                loadPendingRequests();
            }
        }

        // Load my images for encryption
        function loadMyImagesForEncrypt() {
            fetch(API_BASE + '/my-images')
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    var container = document.getElementById('my-images-for-encrypt');
                    if (data.images && data.images.length > 0) {
                        myImages = data.images;
                        var html = '';
                        data.images.forEach(function (img) {
                            html += '<div class="image-card" style="cursor:pointer" onclick="selectImageForEncrypt(\'' + img.encrypted_path + '\', \'' + img.name + '\')">';
                            html += '<h3>' + img.name + '</h3>';
                            html += '<p><strong>ID:</strong> ' + img.image_id + '</p>';
                            html += '<button class="btn-primary btn-small">Select for Encryption</button>';
                            html += '</div>';
                        });
                        container.innerHTML = html;
                    } else {
                        showMessage('encrypt-select-message', 'No images found', 'info');
                    }
                })
                .catch(function (error) {
                    showMessage('encrypt-select-message', 'Error: ' + error.message, 'error');
                });
        }

        function selectImageForEncrypt(imagePath, imageName) {
            selectedImagePath = imagePath;
            selectedImageName = imageName;

            // Load image and show preview
            document.getElementById('preview-img').src = imagePath;
            document.getElementById('preview-filename').textContent = imageName;
            document.getElementById('my-images-for-encrypt').style.display = 'none';
            document.getElementById('image-preview').style.display = 'block';
        }

        function resetEncryptSelection() {
            selectedImagePath = null;
            selectedImageName = null;
            document.getElementById('my-images-for-encrypt').style.display = 'grid';
            document.getElementById('image-preview').style.display = 'none';
            document.getElementById('encrypt-loading').style.display = 'none';
            document.getElementById('encrypted-result').style.display = 'none';
            document.getElementById('decrypted-result').style.display = 'none';
        }

        function encryptSelectedImage() {
            if (!selectedImagePath) return;

            document.getElementById('image-preview').style.display = 'none';
            document.getElementById('encrypt-loading').style.display = 'block';

            // Read the image file from path and send to server
            fetch(selectedImagePath)
                .then(function(response) { return response.blob(); })
                .then(function(blob) {
                    var formData = new FormData();
                    formData.append('image', blob, selectedImageName);

                    return fetch(API_BASE + '/encrypt', {
                        method: 'POST',
                        body: formData
                    });
                })
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    if (data.success && data.carrier_image_base64) {
                        carrierImageBase64 = data.carrier_image_base64;
                        document.getElementById('encrypted-img').src = 'data:image/png;base64,' + carrierImageBase64;
                        document.getElementById('encrypt-loading').style.display = 'none';
                        document.getElementById('encrypted-result').style.display = 'block';
                    } else {
                        throw new Error(data.error || 'Encryption failed');
                    }
                })
                .catch(function (error) {
                    document.getElementById('encrypt-loading').style.display = 'none';
                    showMessage('encrypt-select-message', error.message, 'error');
                    document.getElementById('my-images-for-encrypt').style.display = 'grid';
                });
        }

        function decryptImage() {
            if (!carrierImageBase64) return;

            document.getElementById('encrypted-result').style.display = 'none';
            document.getElementById('encrypt-loading').style.display = 'block';
            document.getElementById('encrypt-loading').querySelector('p').textContent = 'Extracting secret...';

            var byteChars = atob(carrierImageBase64);
            var byteNums = new Array(byteChars.length);
            for (var i = 0; i < byteChars.length; i++) {
                byteNums[i] = byteChars.charCodeAt(i);
            }
            var carrierBlob = new Blob([new Uint8Array(byteNums)], { type: 'image/png' });
            var carrierUrl = URL.createObjectURL(carrierBlob);

            loadImage(carrierUrl)
                .then(function (img) {
                    var canvas = document.createElement('canvas');
                    canvas.width = img.width;
                    canvas.height = img.height;
                    var ctx = canvas.getContext('2d');
                    ctx.drawImage(img, 0, 0);

                    var pixels = ctx.getImageData(0, 0, img.width, img.height);
                    var extractedData = extractLSB(pixels.data);

                    URL.revokeObjectURL(carrierUrl);

                    if (extractedData) {
                        var blob = new Blob([extractedData], { type: 'image/jpeg' });
                        decryptedImageUrl = URL.createObjectURL(blob);
                        document.getElementById('decrypted-img').src = decryptedImageUrl;
                        document.getElementById('encrypt-loading').style.display = 'none';
                        document.getElementById('decrypted-result').style.display = 'block';
                    } else {
                        throw new Error('No hidden data found');
                    }
                })
                .catch(function (error) {
                    document.getElementById('encrypt-loading').style.display = 'none';
                    showMessage('encrypt-select-message', error.message, 'error');
                    document.getElementById('encrypted-result').style.display = 'block';
                });
        }

        function extractLSB(pixelData) {
            try {
                var lengthBits = [];
                for (var i = 0; i < pixelData.length && lengthBits.length < 32; i += 4) {
                    for (var c = 0; c < 3 && lengthBits.length < 32; c++) {
                        lengthBits.push(pixelData[i + c] & 1);
                    }
                }

                var length = 0;
                for (var i = 0; i < 32; i++) {
                    length = (length << 1) | lengthBits[i];
                }

                if (length <= 0 || length > 10000000) throw new Error('Invalid data');

                var dataBits = [];
                var pixelIndex = 0;
                var channelIndex = 0;

                for (var i = 0; i < 32; i++) {
                    channelIndex++;
                    if (channelIndex >= 3) {
                        channelIndex = 0;
                        pixelIndex += 4;
                    }
                }

                while (dataBits.length < length * 8 && pixelIndex < pixelData.length) {
                    dataBits.push(pixelData[pixelIndex + channelIndex] & 1);
                    channelIndex++;
                    if (channelIndex >= 3) {
                        channelIndex = 0;
                        pixelIndex += 4;
                    }
                }

                var dataBytes = new Uint8Array(length);
                for (var i = 0; i < length; i++) {
                    var byte = 0;
                    for (var bit = 0; bit < 8; bit++) {
                        byte = (byte << 1) | dataBits[i * 8 + bit];
                    }
                    dataBytes[i] = byte;
                }

                return dataBytes;
            } catch (error) {
                console.error('Extraction error:', error);
                return null;
            }
        }

        function loadImage(src) {
            return new Promise(function (resolve, reject) {
                var img = new Image();
                img.onload = function () { resolve(img); };
                img.onerror = reject;
                img.src = src;
            });
        }

        // DoS functions
        function refreshOnlinePeers() {
            document.getElementById('dos-loading').style.display = 'block';
            document.getElementById('peer-list').innerHTML = '';
            document.getElementById('dos-message').innerHTML = '';

            fetch(API_BASE + '/online-peers')
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    document.getElementById('dos-loading').style.display = 'none';
                    if (data.peers && data.peers.length > 0) {
                        var html = '';
                        data.peers.forEach(function (peer) {
                            html += '<div class="peer-card">';
                            html += '<h3>' + peer.client_id + ' <span class="badge badge-online">ONLINE</span></h3>';
                            html += '<p><strong>Name:</strong> ' + (peer.client_name || '-') + '</p>';
                            html += '<p><strong>IP:</strong> ' + peer.ip_address + '</p>';
                            html += '<p><strong>Images:</strong></p>';
                            if (peer.images && Object.keys(peer.images).length > 0) {
                                html += '<ul style="margin-left: 20px;">';
                                for (var imgId in peer.images) {
                                    html += '<li>' + peer.images[imgId].name + ' (ID: ' + imgId + ')</li>';
                                }
                                html += '</ul>';
                            } else {
                                html += '<p style="margin-left: 20px; color: #999;">No images</p>';
                            }
                            html += '</div>';
                        });
                        document.getElementById('peer-list').innerHTML = html;
                    } else {
                        showMessage('dos-message', 'No online peers found', 'info');
                    }
                })
                .catch(function (error) {
                    document.getElementById('dos-loading').style.display = 'none';
                    showMessage('dos-message', 'Error: ' + error.message, 'error');
                });
        }

        // Request image access
        function requestImageAccess() {
            var ownerId = document.getElementById('request-owner-id').value.trim();
            var imageId = document.getElementById('request-image-id').value.trim();

            if (!ownerId || !imageId) {
                showMessage('request-message', 'Please fill all fields', 'error');
                return;
            }

            fetch(API_BASE + '/request-access', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    owner_id: ownerId,
                    image_id: imageId
                })
            })
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    if (data.success) {
                        showMessage('request-message', 'Access request sent! Request ID: ' + data.request_id, 'success');
                        document.getElementById('request-owner-id').value = '';
                        document.getElementById('request-image-id').value = '';
                        setTimeout(loadRequestedImages, 1000);
                    } else {
                        showMessage('request-message', data.error || 'Request failed', 'error');
                    }
                })
                .catch(function (error) {
                    showMessage('request-message', 'Error: ' + error.message, 'error');
                });
        }

        // Load requested images
        function loadRequestedImages() {
            if (!currentUser) return;

            document.getElementById('requested-loading').style.display = 'block';
            document.getElementById('requested-list').innerHTML = '';
            document.getElementById('requested-message').innerHTML = '';

            fetch(API_BASE + '/requested-images')
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    document.getElementById('requested-loading').style.display = 'none';
                    if (data.images && data.images.length > 0) {
                        var html = '';
                        data.images.forEach(function (img) {
                            html += '<div class="image-card">';
                            html += '<h3>' + img.name + '</h3>';
                            html += '<p><strong>Owner:</strong> ' + img.owner_id + '</p>';
                            html += '<p><strong>Status:</strong> ' + (img.has_access ? '<span class="badge badge-online">GRANTED</span>' : '<span class="badge badge-offline">PENDING</span>') + '</p>';
                            if (img.has_access) {
                                html += '<button class="btn-success btn-small" onclick="viewRequestedImage(\'' + img.image_id + '\')">View/Decrypt</button>';
                            }
                            html += '</div>';
                        });
                        document.getElementById('requested-list').innerHTML = html;
                    } else {
                        showMessage('requested-message', 'No requested images', 'info');
                    }
                })
                .catch(function (error) {
                    document.getElementById('requested-loading').style.display = 'none';
                    showMessage('requested-message', 'Error: ' + error.message, 'error');
                });
        }

        function viewRequestedImage(imageId) {
            // Fetch and decrypt the image
            fetch(API_BASE + '/view-image/' + imageId)
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    if (data.success && data.image_data) {
                        // Show decrypted image
                        alert('Image decrypted successfully!');
                    } else {
                        alert('Failed to decrypt image: ' + (data.error || 'Unknown error'));
                    }
                })
                .catch(function (error) {
                    alert('Error: ' + error.message);
                });
        }

        // Load my images
        function loadMyImages() {
            document.getElementById('shared-loading').style.display = 'block';
            document.getElementById('shared-list').innerHTML = '';
            document.getElementById('shared-message').innerHTML = '';

            fetch(API_BASE + '/my-images')
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    document.getElementById('shared-loading').style.display = 'none';
                    if (data.images && data.images.length > 0) {
                        var html = '';
                        data.images.forEach(function (img) {
                            html += '<div class="image-card">';
                            html += '<h3>' + img.name + '</h3>';
                            html += '<p><strong>ID:</strong> ' + img.image_id + '</p>';
                            var accessText = img.access_rights && img.access_rights.length > 0 ? img.access_rights.join(', ') : 'Owner (You) + ' + (img.access_rights && img.access_rights.length > 0 ? img.access_rights.join(', ') : 'None');
                            html += '<p><strong>Access rights:</strong> ' + accessText + '</p>';
                            html += '</div>';
                        });
                        document.getElementById('shared-list').innerHTML = html;
                    } else {
                        showMessage('shared-message', 'No images found', 'info');
                    }
                })
                .catch(function (error) {
                    document.getElementById('shared-loading').style.display = 'none';
                    showMessage('shared-message', 'Error: ' + error.message, 'error');
                });
        }

        // Update access rights
        function updateAccessRights() {
            var imageId = document.getElementById('access-image-id').value.trim();
            var clientIds = document.getElementById('access-client-ids').value.trim();

            if (!imageId) {
                showMessage('access-message', 'Please enter an image ID', 'error');
                return;
            }

            var accessList = clientIds ? clientIds.split(',').map(function (s) { return s.trim(); }) : [];

            fetch(API_BASE + '/update-access', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    image_id: imageId,
                    access_list: accessList
                })
            })
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    if (data.success) {
                        showMessage('access-message', 'Access rights updated successfully!', 'success');
                        document.getElementById('access-image-id').value = '';
                        document.getElementById('access-client-ids').value = '';
                        setTimeout(loadMyImages, 1000);
                    } else {
                        showMessage('access-message', data.error || 'Update failed', 'error');
                    }
                })
                .catch(function (error) {
                    showMessage('access-message', 'Error: ' + error.message, 'error');
                });
        }

        // Load pending requests
        function loadPendingRequests() {
            if (!currentUser) return;

            document.getElementById('pending-loading').style.display = 'block';
            document.getElementById('pending-list').innerHTML = '';
            document.getElementById('pending-message').innerHTML = '';

            fetch(API_BASE + '/pending-requests')
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    document.getElementById('pending-loading').style.display = 'none';
                    if (data.requests && data.requests.length > 0) {
                        var badge = document.getElementById('pending-count');
                        badge.textContent = data.requests.length;
                        badge.style.display = 'inline-block';

                        var html = '';
                        data.requests.forEach(function (req) {
                            html += '<div class="request-card">';
                            html += '<h3>Request from ' + req.requester_id + '</h3>';
                            html += '<p><strong>Image ID:</strong> ' + req.image_id + '</p>';
                            html += '<p><strong>Request ID:</strong> ' + req.request_id + '</p>';
                            html += '<div class="button-group">';
                            html += '<button class="btn-success btn-small" onclick="respondToRequest(\'' + req.request_id + '\', true)">Approve</button>';
                            html += '<button class="btn-danger btn-small" onclick="respondToRequest(\'' + req.request_id + '\', false)">Deny</button>';
                            html += '</div>';
                            html += '</div>';
                        });
                        document.getElementById('pending-list').innerHTML = html;
                    } else {
                        document.getElementById('pending-count').style.display = 'none';
                        showMessage('pending-message', 'No pending requests', 'info');
                    }
                })
                .catch(function (error) {
                    document.getElementById('pending-loading').style.display = 'none';
                    showMessage('pending-message', 'Error: ' + error.message, 'error');
                });
        }

        // Respond to access request
        function respondToRequest(requestId, approved) {
            fetch(API_BASE + '/respond-request', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    request_id: requestId,
                    approved: approved
                })
            })
                .then(function (response) { return response.json(); })
                .then(function (data) {
                    if (data.success) {
                        showMessage('pending-message', 'Response sent successfully!', 'success');
                        setTimeout(loadPendingRequests, 1000);
                    } else {
                        showMessage('pending-message', data.error || 'Response failed', 'error');
                    }
                })
                .catch(function (error) {
                    showMessage('pending-message', 'Error: ' + error.message, 'error');
                });
        }

        // Utility function
        function showMessage(elementId, message, type) {
            var element = document.getElementById(elementId);
            element.innerHTML = '<div class="message ' + type + '">' + message + '</div>';
            setTimeout(function () {
                element.innerHTML = '';
            }, 5000);
        }

        // Auto-login if user exists
        window.onload = function () {
            var savedUser = localStorage.getItem('currentUser');
            if (savedUser) {
                currentUser = savedUser;
                showMainSection();
                startAutoRefresh();
            }
        };
    </script>
